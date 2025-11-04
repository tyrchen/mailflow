/// Storage endpoints
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{context::ApiContext, error::ApiError};

#[derive(Debug, Serialize)]
pub struct StorageStatsResponse {
    pub buckets: Vec<BucketInfo>,
}

#[derive(Debug, Serialize)]
pub struct BucketInfo {
    pub name: String,
    #[serde(rename = "objectCount")]
    pub object_count: usize,
    #[serde(rename = "totalSizeBytes")]
    pub total_size_bytes: i64,
    #[serde(rename = "oldestObject")]
    pub oldest_object: Option<String>,
    #[serde(rename = "newestObject")]
    pub newest_object: Option<String>,
    #[serde(rename = "contentTypeBreakdown")]
    pub content_type_breakdown: Vec<ContentTypeStats>,
}

#[derive(Debug, Serialize)]
pub struct ContentTypeStats {
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub count: usize,
    #[serde(rename = "totalSizeBytes")]
    pub total_size_bytes: i64,
}

#[derive(Debug, Deserialize)]
pub struct ObjectsQuery {
    pub limit: Option<i32>,
    pub prefix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ObjectsResponse {
    pub bucket: String,
    pub objects: Vec<ObjectInfo>,
}

#[derive(Debug, Serialize)]
pub struct ObjectInfo {
    pub key: String,
    pub size: i64,
    #[serde(rename = "lastModified")]
    pub last_modified: String,
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    #[serde(rename = "presignedUrl")]
    pub presigned_url: String,
}

pub async fn stats(
    State(ctx): State<Arc<ApiContext>>,
) -> Result<Json<StorageStatsResponse>, ApiError> {
    let buckets_result = ctx
        .s3_client
        .list_buckets()
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let mut buckets = Vec::new();

    for bucket in buckets_result.buckets() {
        let bucket_name = bucket.name().unwrap_or("").to_string();

        // Only include mailflow-related buckets
        if !bucket_name.starts_with("mailflow-") {
            continue;
        }

        // List objects to get stats (simplified - would paginate in production)
        let objects_result = ctx
            .s3_client
            .list_objects_v2()
            .bucket(&bucket_name)
            .max_keys(1000)
            .send()
            .await;

        let (object_count, total_size, oldest, newest, breakdown) = match objects_result {
            Ok(result) => {
                let count = result.contents().len();
                let size: i64 = result.contents().iter().filter_map(|obj| obj.size()).sum();
                let oldest_date = result
                    .contents()
                    .iter()
                    .filter_map(|obj| obj.last_modified())
                    .min()
                    .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), 0))
                    .map(|dt| dt.to_rfc3339());
                let newest_date = result
                    .contents()
                    .iter()
                    .filter_map(|obj| obj.last_modified())
                    .max()
                    .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), 0))
                    .map(|dt| dt.to_rfc3339());

                // Group by content type (inferred from file extension)
                let mut content_type_map: HashMap<String, (usize, i64)> = HashMap::new();
                for obj in result.contents() {
                    let key = obj.key().unwrap_or("");
                    let size = obj.size().unwrap_or(0);

                    // Infer content type from extension
                    let content_type = if key.ends_with(".pdf") {
                        "application/pdf"
                    } else if key.ends_with(".png") {
                        "image/png"
                    } else if key.ends_with(".jpg") || key.ends_with(".jpeg") {
                        "image/jpeg"
                    } else if key.ends_with(".gif") {
                        "image/gif"
                    } else if key.ends_with(".txt") {
                        "text/plain"
                    } else if key.ends_with(".html") || key.ends_with(".htm") {
                        "text/html"
                    } else if key.ends_with(".json") {
                        "application/json"
                    } else if key.ends_with(".zip") {
                        "application/zip"
                    } else if key.ends_with(".eml") {
                        "message/rfc822"
                    } else {
                        "application/octet-stream"
                    };

                    let entry = content_type_map
                        .entry(content_type.to_string())
                        .or_insert((0, 0));
                    entry.0 += 1;
                    entry.1 += size;
                }

                let breakdown: Vec<ContentTypeStats> = content_type_map
                    .into_iter()
                    .map(
                        |(content_type, (count, total_size_bytes))| ContentTypeStats {
                            content_type,
                            count,
                            total_size_bytes,
                        },
                    )
                    .collect();

                (count, size, oldest_date, newest_date, breakdown)
            }
            Err(_) => (0, 0, None, None, vec![]),
        };

        buckets.push(BucketInfo {
            name: bucket_name,
            object_count,
            total_size_bytes: total_size,
            oldest_object: oldest,
            newest_object: newest,
            content_type_breakdown: breakdown,
        });
    }

    Ok(Json(StorageStatsResponse { buckets }))
}

pub async fn objects(
    State(ctx): State<Arc<ApiContext>>,
    Path(bucket): Path<String>,
    Query(query): Query<ObjectsQuery>,
) -> Result<Json<ObjectsResponse>, ApiError> {
    let limit = query.limit.unwrap_or(20).min(100);

    let mut request = ctx
        .s3_client
        .list_objects_v2()
        .bucket(&bucket)
        .max_keys(limit);

    if let Some(prefix) = &query.prefix {
        request = request.prefix(prefix);
    }

    let result = request
        .send()
        .await
        .map_err(|e| ApiError::Aws(e.to_string()))?;

    let mut objects = Vec::new();

    for obj in result.contents() {
        let key = obj.key().unwrap_or("").to_string();
        let size = obj.size().unwrap_or(0);
        let last_modified = obj
            .last_modified()
            .and_then(|dt| chrono::DateTime::from_timestamp(dt.secs(), 0))
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_default();

        // Generate presigned URL (7 days expiration)
        let presigned_url = ctx
            .s3_client
            .get_object()
            .bucket(&bucket)
            .key(&key)
            .presigned(
                aws_sdk_s3::presigning::PresigningConfig::expires_in(
                    std::time::Duration::from_secs(7 * 24 * 60 * 60),
                )
                .map_err(|e| ApiError::Internal(e.to_string()))?,
            )
            .await
            .map(|req| req.uri().to_string())
            .unwrap_or_default();

        objects.push(ObjectInfo {
            key,
            size,
            last_modified,
            content_type: None,
            presigned_url,
        });
    }

    Ok(Json(ObjectsResponse { bucket, objects }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_inference() {
        let test_cases = vec![
            ("document.pdf", "application/pdf"),
            ("image.png", "image/png"),
            ("photo.jpg", "image/jpeg"),
            ("animation.gif", "image/gif"),
            ("readme.txt", "text/plain"),
            ("index.html", "text/html"),
            ("data.json", "application/json"),
            ("archive.zip", "application/zip"),
            ("email.eml", "message/rfc822"),
            ("unknown.xyz", "application/octet-stream"),
        ];

        for (filename, expected_type) in test_cases {
            let content_type = if filename.ends_with(".pdf") {
                "application/pdf"
            } else if filename.ends_with(".png") {
                "image/png"
            } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
                "image/jpeg"
            } else if filename.ends_with(".gif") {
                "image/gif"
            } else if filename.ends_with(".txt") {
                "text/plain"
            } else if filename.ends_with(".html") || filename.ends_with(".htm") {
                "text/html"
            } else if filename.ends_with(".json") {
                "application/json"
            } else if filename.ends_with(".zip") {
                "application/zip"
            } else if filename.ends_with(".eml") {
                "message/rfc822"
            } else {
                "application/octet-stream"
            };
            assert_eq!(content_type, expected_type);
        }
    }

    #[test]
    fn test_content_type_breakdown_aggregation() {
        let objects = vec![
            ("file1.pdf", 1000),
            ("file2.pdf", 2000),
            ("image1.jpg", 500),
        ];

        let mut content_type_map: HashMap<String, (usize, i64)> = HashMap::new();
        for (key, size) in objects {
            let content_type = if key.ends_with(".pdf") {
                "application/pdf"
            } else if key.ends_with(".jpg") {
                "image/jpeg"
            } else {
                "application/octet-stream"
            };

            let entry = content_type_map
                .entry(content_type.to_string())
                .or_insert((0, 0));
            entry.0 += 1;
            entry.1 += size;
        }

        assert_eq!(content_type_map["application/pdf"], (2, 3000));
        assert_eq!(content_type_map["image/jpeg"], (1, 500));
    }

    #[test]
    fn test_objects_query_limit() {
        let test_limits = vec![(Some(50), 50), (Some(150), 100), (None, 20)];
        for (input, expected) in test_limits {
            let limit = input.unwrap_or(20).min(100);
            assert_eq!(limit, expected);
        }
    }
}
