import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export function createLambdaRole(
    environment: string,
    bucketArn: pulumi.Output<string>,
    attachmentsBucketArn: pulumi.Output<string>,
    queueArns: pulumi.Output<string>[],
    idempotencyTableArn: pulumi.Output<string>
) {
    // IAM role for Lambda
    const lambdaRole = new aws.iam.Role(`mailflow-lambda-role-${environment}`, {
        name: `mailflow-lambda-role-${environment}`,
        assumeRolePolicy: JSON.stringify({
            Version: "2012-10-17",
            Statement: [
                {
                    Action: "sts:AssumeRole",
                    Effect: "Allow",
                    Principal: {
                        Service: "lambda.amazonaws.com",
                    },
                },
            ],
        }),
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Lambda execution policy
    const lambdaPolicy = new aws.iam.RolePolicy(`mailflow-lambda-policy-${environment}`, {
        role: lambdaRole.id,
        policy: pulumi
            .all([bucketArn, attachmentsBucketArn, queueArns, idempotencyTableArn])
            .apply(([bucket, attachmentsBucket, queues, table]) =>
                JSON.stringify({
                    Version: "2012-10-17",
                    Statement: [
                        {
                            Sid: "S3Access",
                            Effect: "Allow",
                            Action: ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"],
                            Resource: `${bucket}/*`,
                        },
                        {
                            Sid: "AttachmentsBucketAccess",
                            Effect: "Allow",
                            Action: ["s3:GetObject", "s3:PutObject", "s3:DeleteObject"],
                            Resource: `${attachmentsBucket}/*`,
                        },
                        {
                            Sid: "SQSAccess",
                            Effect: "Allow",
                            Action: [
                                "sqs:SendMessage",
                                "sqs:ReceiveMessage",
                                "sqs:DeleteMessage",
                                "sqs:GetQueueAttributes",
                            ],
                            Resource: queues,
                        },
                        {
                            Sid: "SESAccess",
                            Effect: "Allow",
                            Action: ["ses:SendRawEmail", "ses:GetSendQuota"],
                            Resource: "*",
                        },
                        {
                            Sid: "DynamoDBAccess",
                            Effect: "Allow",
                            Action: [
                                "dynamodb:GetItem",
                                "dynamodb:PutItem",
                                "dynamodb:Query",
                            ],
                            Resource: table,
                        },
                        {
                            Sid: "CloudWatchLogs",
                            Effect: "Allow",
                            Action: [
                                "logs:CreateLogGroup",
                                "logs:CreateLogStream",
                                "logs:PutLogEvents",
                            ],
                            Resource: "*",
                        },
                        {
                            Sid: "KMSAccess",
                            Effect: "Allow",
                            Action: [
                                "kms:Decrypt",
                                "kms:Encrypt",
                                "kms:GenerateDataKey",
                            ],
                            Resource: "*",
                        },
                        {
                            Sid: "CloudWatchMetrics",
                            Effect: "Allow",
                            Action: [
                                "cloudwatch:PutMetricData",
                            ],
                            Resource: "*",
                            Condition: {
                                StringEquals: {
                                    "cloudwatch:namespace": "Mailflow",
                                },
                            },
                        },
                    ],
                })
            ),
    });

    return {
        role: lambdaRole,
        policy: lambdaPolicy,
    };
}
