import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export interface LambdaConfig {
    role: aws.iam.Role;
    rawEmailsBucket: aws.s3.Bucket;
    attachmentsBucket: aws.s3.Bucket;
    appQueues: Record<string, aws.sqs.Queue>;
    outboundQueue: aws.sqs.Queue;
    defaultQueue: aws.sqs.Queue;
    dlq: aws.sqs.Queue;
    idempotencyTable: aws.dynamodb.Table;
    domains: string[];
    allowedSenderDomains: string[];
    environment: string;
}

export function createLambdaFunction(config: LambdaConfig) {
    const { role, rawEmailsBucket, attachmentsBucket, appQueues, outboundQueue, defaultQueue, dlq, idempotencyTable, domains, allowedSenderDomains, environment } =
        config;

    // Build routing map from app queues
    const routingMap: Record<string, pulumi.Output<string>> = {};
    for (const [appName, queue] of Object.entries(appQueues)) {
        routingMap[appName] = queue.url;
    }

    const routingMapJson = pulumi.output(routingMap).apply((map) => {
        const resolved: Record<string, string> = {};
        for (const [key, value] of Object.entries(map)) {
            resolved[key] = value;
        }
        return JSON.stringify(resolved);
    });

    // Lambda function
    const lambdaFunction = new aws.lambda.Function(`mailflow-${environment}`, {
        name: `mailflow-${environment}`,
        runtime: "provided.al2023",
        handler: "bootstrap",
        role: role.arn,
        timeout: 60,
        memorySize: 256,
        architectures: ["arm64"], // Use ARM64 for better cost efficiency
        code: new pulumi.asset.FileArchive("../assets/bootstrap.zip"),
        environment: {
            variables: {
                RUST_LOG: "info",
                ROUTING_MAP: routingMapJson,
                IDEMPOTENCY_TABLE: idempotencyTable.name,
                RAW_EMAILS_BUCKET: rawEmailsBucket.bucket,
                ATTACHMENTS_BUCKET: attachmentsBucket.bucket,
                OUTBOUND_QUEUE_URL: outboundQueue.url,
                DEFAULT_QUEUE_URL: defaultQueue.url,
                DLQ_URL: dlq.url,
                ALLOWED_DOMAINS: domains.join(","),
                ALLOWED_SENDER_DOMAINS: allowedSenderDomains.join(","),
                PRESIGNED_URL_EXPIRATION_SECONDS: "604800",
                MAX_ATTACHMENT_SIZE_BYTES: "36700160",
                ALLOWED_CONTENT_TYPES: "*",
                BLOCKED_CONTENT_TYPES: "application/x-executable,application/x-msdownload",
            },
        },
        deadLetterConfig: {
            targetArn: dlq.arn,
        },
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // CloudWatch Log Group
    const logGroup = new aws.cloudwatch.LogGroup(`mailflow-logs-${environment}`, {
        name: pulumi.interpolate`/aws/lambda/${lambdaFunction.name}`,
        retentionInDays: 30,
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // SQS Event Source Mapping for outbound queue
    const sqsEventSource = new aws.lambda.EventSourceMapping(
        `mailflow-outbound-trigger-${environment}`,
        {
            eventSourceArn: outboundQueue.arn,
            functionName: lambdaFunction.name,
            batchSize: 10,
            maximumBatchingWindowInSeconds: 5,
        }
    );

    return {
        function: lambdaFunction,
        logGroup,
        sqsEventSource,
    };
}
