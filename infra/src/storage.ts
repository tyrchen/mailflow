import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export function createStorage(environment: string) {
    // S3 bucket for attachments
    const attachmentsBucket = new aws.s3.Bucket(`mailflow-attachments-${environment}`, {
        bucket: `mailflow-attachments-${environment}`,
        lifecycleRules: [
            {
                enabled: true,
                expiration: {
                    days: 30, // Delete attachments after 30 days
                },
            },
        ],
        serverSideEncryptionConfiguration: {
            rule: {
                applyServerSideEncryptionByDefault: {
                    sseAlgorithm: "AES256",
                },
            },
        },
        corsRules: [
            {
                allowedMethods: ["GET"],
                allowedOrigins: ["*"],
                allowedHeaders: ["*"],
                maxAgeSeconds: 3600,
            },
        ],
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // S3 bucket for raw emails
    const rawEmailsBucket = new aws.s3.Bucket(`mailflow-raw-emails-${environment}`, {
        bucket: `mailflow-raw-emails-${environment}`,
        lifecycleRules: [
            {
                enabled: true,
                expiration: {
                    days: 7, // Delete raw emails after 7 days
                },
            },
        ],
        serverSideEncryptionConfiguration: {
            rule: {
                applyServerSideEncryptionByDefault: {
                    sseAlgorithm: "AES256",
                },
            },
        },
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Block public access
    const publicAccessBlock = new aws.s3.BucketPublicAccessBlock(`mailflow-raw-emails-public-access-block-${environment}`, {
        bucket: rawEmailsBucket.id,
        blockPublicAcls: true,
        blockPublicPolicy: true,
        ignorePublicAcls: true,
        restrictPublicBuckets: true,
    });

    // Grant SES permission to write to bucket
    const bucketPolicy = new aws.s3.BucketPolicy(`mailflow-raw-emails-policy-${environment}`, {
        bucket: rawEmailsBucket.id,
        policy: pulumi.all([rawEmailsBucket.arn]).apply(([bucketArn]) =>
            JSON.stringify({
                Version: "2012-10-17",
                Statement: [
                    {
                        Sid: "AllowSESPuts",
                        Effect: "Allow",
                        Principal: {
                            Service: "ses.amazonaws.com",
                        },
                        Action: "s3:PutObject",
                        Resource: `${bucketArn}/*`,
                    },
                ],
            })
        ),
    });

    return {
        bucket: rawEmailsBucket,
        attachmentsBucket,
        bucketPolicy,
        publicAccessBlock,
    };
}
