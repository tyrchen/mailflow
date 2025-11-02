import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export function createStorage(environment: string) {
    // S3 bucket for attachments
    const attachmentsBucket = new aws.s3.Bucket(`mailflow-attachments-${environment}`, {
        bucket: `mailflow-attachments-${environment}`,
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Lifecycle configuration for attachments bucket
    const attachmentsLifecycle = new aws.s3.BucketLifecycleConfiguration(`mailflow-attachments-lifecycle-${environment}`, {
        bucket: attachmentsBucket.id,
        rules: [
            {
                id: "delete-old-attachments",
                status: "Enabled",
                expiration: {
                    days: 30, // Delete attachments after 30 days
                },
            },
        ],
    });

    // Server-side encryption configuration for attachments bucket
    const attachmentsEncryption = new aws.s3.BucketServerSideEncryptionConfiguration(`mailflow-attachments-encryption-${environment}`, {
        bucket: attachmentsBucket.id,
        rules: [
            {
                applyServerSideEncryptionByDefault: {
                    sseAlgorithm: "AES256",
                },
            },
        ],
    });

    // CORS configuration for attachments bucket
    const attachmentsCors = new aws.s3.BucketCorsConfiguration(`mailflow-attachments-cors-${environment}`, {
        bucket: attachmentsBucket.id,
        corsRules: [
            {
                allowedMethods: ["GET"],
                allowedOrigins: ["*"],
                allowedHeaders: ["*"],
                maxAgeSeconds: 3600,
            },
        ],
    });

    // S3 bucket for raw emails
    const rawEmailsBucket = new aws.s3.Bucket(`mailflow-raw-emails-${environment}`, {
        bucket: `mailflow-raw-emails-${environment}`,
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Lifecycle configuration for raw emails bucket
    const rawEmailsLifecycle = new aws.s3.BucketLifecycleConfiguration(`mailflow-raw-emails-lifecycle-${environment}`, {
        bucket: rawEmailsBucket.id,
        rules: [
            {
                id: "delete-old-emails",
                status: "Enabled",
                expiration: {
                    days: 7, // Delete raw emails after 7 days
                },
            },
        ],
    });

    // Server-side encryption configuration for raw emails bucket
    const rawEmailsEncryption = new aws.s3.BucketServerSideEncryptionConfiguration(`mailflow-raw-emails-encryption-${environment}`, {
        bucket: rawEmailsBucket.id,
        rules: [
            {
                applyServerSideEncryptionByDefault: {
                    sseAlgorithm: "AES256",
                },
            },
        ],
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
        attachmentsLifecycle,
        attachmentsEncryption,
        attachmentsCors,
        rawEmailsLifecycle,
        rawEmailsEncryption,
    };
}
