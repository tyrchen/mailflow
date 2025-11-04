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

export function createApiLambdaRole(environment: string) {
    // IAM role for API Lambda
    const apiLambdaRole = new aws.iam.Role(`mailflow-api-lambda-role-${environment}`, {
        name: `mailflow-api-lambda-role-${environment}`,
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
            Service: "mailflow-api",
        },
    });

    // API Lambda execution policy - read-only access to AWS resources
    const apiLambdaPolicy = new aws.iam.RolePolicy(`mailflow-api-lambda-policy-${environment}`, {
        role: apiLambdaRole.id,
        policy: JSON.stringify({
            Version: "2012-10-17",
            Statement: [
                {
                    Sid: "CloudWatchLogsAccess",
                    Effect: "Allow",
                    Action: [
                        "logs:CreateLogGroup",
                        "logs:CreateLogStream",
                        "logs:PutLogEvents",
                        "logs:FilterLogEvents",
                        "logs:DescribeLogGroups",
                        "logs:DescribeLogStreams",
                        "logs:GetLogEvents",
                    ],
                    Resource: "*",
                },
                {
                    Sid: "S3ReadAccess",
                    Effect: "Allow",
                    Action: [
                        "s3:ListAllMyBuckets",
                        "s3:ListBucket",
                        "s3:GetObject",
                        "s3:GetObjectAttributes",
                        "s3:ListBucketVersions",
                        "s3:GetBucketLocation",
                    ],
                    Resource: "*",
                },
                {
                    Sid: "SQSAccess",
                    Effect: "Allow",
                    Action: [
                        "sqs:GetQueueAttributes",
                        "sqs:ReceiveMessage",
                        "sqs:ListQueues",
                        "sqs:GetQueueUrl",
                        "sqs:SendMessage",
                    ],
                    Resource: "*",
                },
                {
                    Sid: "CloudWatchMetricsRead",
                    Effect: "Allow",
                    Action: [
                        "cloudwatch:GetMetricData",
                        "cloudwatch:GetMetricStatistics",
                        "cloudwatch:ListMetrics",
                        "cloudwatch:PutMetricData",
                    ],
                    Resource: "*",
                },
                {
                    Sid: "DynamoDBAccess",
                    Effect: "Allow",
                    Action: [
                        "dynamodb:GetItem",
                        "dynamodb:Query",
                        "dynamodb:Scan",
                        "dynamodb:DescribeTable",
                        "dynamodb:PutItem",
                        "dynamodb:UpdateItem",
                    ],
                    Resource: "*",
                },
                {
                    Sid: "SESTestEmailAccess",
                    Effect: "Allow",
                    Action: [
                        "ses:SendEmail",
                        "ses:SendRawEmail",
                    ],
                    Resource: "*",
                },
            ],
        }),
    });

    return {
        role: apiLambdaRole,
        policy: apiLambdaPolicy,
    };
}
