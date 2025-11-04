/// Storage endpoints
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
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

        // List objects to get stats (simplified - would paginate in production)
        let objects_result = ctx
            .s3_client
            .list_objects_v2()
            .bucket(&bucket_name)
            .max_keys(1000)
            .send()
            .await;

        let (object_count, total_size, oldest, newest) = match objects_result {
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

                (count, size, oldest_date, newest_date)
            }
            Err(_) => (0, 0, None, None),
        };

        buckets.push(BucketInfo {
            name: bucket_name,
            object_count,
            total_size_bytes: total_size,
            oldest_object: oldest,
            newest_object: newest,
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
