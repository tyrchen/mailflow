import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";

export interface DashboardConfig {
    apiUrl: pulumi.Output<string>;
    environment: string;
}

export function createDashboard(config: DashboardConfig) {
    const { apiUrl, environment } = config;

    // 1. S3 Bucket for dashboard static assets
    const bucket = new aws.s3.Bucket(`mailflow-dashboard-${environment}`, {
        bucket: `mailflow-dashboard-${environment}`,
        website: {
            indexDocument: "index.html",
            errorDocument: "index.html", // SPA routing
        },
        corsRules: [
            {
                allowedHeaders: ["*"],
                allowedMethods: ["GET", "HEAD"],
                allowedOrigins: ["*"],
                exposeHeaders: ["ETag"],
                maxAgeSeconds: 3000,
            },
        ],
    });

    // 2. Block public access (CloudFront will access via OAI)
    const bucketPublicAccessBlock = new aws.s3.BucketPublicAccessBlock(
        `dashboard-public-access-block-${environment}`,
        {
            bucket: bucket.id,
            blockPublicAcls: true,
            blockPublicPolicy: true,
            ignorePublicAcls: true,
            restrictPublicBuckets: true,
        }
    );

    // 3. CloudFront Origin Access Identity
    const oai = new aws.cloudfront.OriginAccessIdentity(`dashboard-oai-${environment}`, {
        comment: `OAI for mailflow-dashboard-${environment}`,
    });

    // 4. Bucket policy to allow CloudFront OAI access
    const bucketPolicy = new aws.s3.BucketPolicy(
        `dashboard-policy-${environment}`,
        {
            bucket: bucket.id,
            policy: pulumi
                .all([bucket.arn, oai.iamArn])
                .apply(([bucketArn, oaiArn]) =>
                    JSON.stringify({
                        Version: "2012-10-17",
                        Statement: [
                            {
                                Effect: "Allow",
                                Principal: {
                                    AWS: oaiArn,
                                },
                                Action: "s3:GetObject",
                                Resource: `${bucketArn}/*`,
                            },
                        ],
                    })
                ),
        },
        { dependsOn: [bucketPublicAccessBlock] }
    );

    // 5. CloudFront distribution
    const cdn = new aws.cloudfront.Distribution(`dashboard-cdn-${environment}`, {
        enabled: true,
        comment: `Mailflow Dashboard ${environment}`,
        defaultRootObject: "index.html",

        // Origins
        origins: [
            {
                // S3 origin for static assets
                originId: "s3-dashboard",
                domainName: bucket.bucketRegionalDomainName,
                s3OriginConfig: {
                    originAccessIdentity: oai.cloudfrontAccessIdentityPath,
                },
            },
        ],

        // Default cache behavior (static assets)
        defaultCacheBehavior: {
            targetOriginId: "s3-dashboard",
            viewerProtocolPolicy: "redirect-to-https",
            allowedMethods: ["GET", "HEAD", "OPTIONS"],
            cachedMethods: ["GET", "HEAD"],
            forwardedValues: {
                queryString: false,
                cookies: {
                    forward: "none",
                },
            },
            minTtl: 0,
            defaultTtl: 86400,
            maxTtl: 31536000,
            compress: true,
        },

        // Custom error responses for SPA routing
        customErrorResponses: [
            {
                errorCode: 404,
                responseCode: 200,
                responsePagePath: "/index.html",
                errorCachingMinTtl: 300,
            },
            {
                errorCode: 403,
                responseCode: 200,
                responsePagePath: "/index.html",
                errorCachingMinTtl: 300,
            },
        ],

        restrictions: {
            geoRestriction: {
                restrictionType: "none",
            },
        },

        viewerCertificate: {
            cloudfrontDefaultCertificate: true,
        },

        priceClass: "PriceClass_100", // Use only North America and Europe
    });

    return {
        bucket,
        cdn,
        dashboardUrl: cdn.domainName,
    };
}
