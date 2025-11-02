import * as pulumi from "@pulumi/pulumi";
import { createStorage } from "./storage";
import { createQueues } from "./queues";
import { createDatabaseTables } from "./database";
import { createLambdaRole } from "./iam";
import { createLambdaFunction } from "./lambda";
import { createSesConfiguration } from "./ses";
import { createMonitoring } from "./monitoring";

// Load configuration
const config = new pulumi.Config();
const environment = config.require("environment");
const domains = config.requireObject<string[]>("domains");
const apps = config.requireObject<string[]>("apps");

console.log(`Deploying Mailflow infrastructure for environment: ${environment}`);
console.log(`Apps: ${apps.join(", ")}`);
console.log(`Domains: ${domains.join(", ")}`);

// 1. Create S3 storage
const storage = createStorage(environment);

// 2. Create SQS queues
const queues = createQueues(environment, apps);

// 3. Create DynamoDB tables
const database = createDatabaseTables(environment);

// 4. Create IAM role for Lambda
const allQueueArns = [
    queues.outboundQueue.arn,
    queues.defaultQueue.arn,
    queues.dlq.arn,
    ...Object.values(queues.appQueues).map((q) => q.arn),
];

const iam = createLambdaRole(
    environment,
    storage.bucket.arn,
    storage.attachmentsBucket.arn,
    allQueueArns,
    database.idempotencyTable.arn
);

// 5. Create Lambda function
const lambda = createLambdaFunction({
    role: iam.role,
    rawEmailsBucket: storage.bucket,
    attachmentsBucket: storage.attachmentsBucket,
    appQueues: queues.appQueues,
    outboundQueue: queues.outboundQueue,
    defaultQueue: queues.defaultQueue,
    dlq: queues.dlq,
    idempotencyTable: database.idempotencyTable,
    domains,
    environment,
});

// 6. Configure SES
const ses = createSesConfiguration({
    lambdaFunction: lambda.function,
    rawEmailsBucket: storage.bucket,
    domains,
    environment,
});

// 7. Create monitoring and alarms
const monitoring = createMonitoring({
    lambdaFunction: lambda.function,
    dlq: queues.dlq,
    environment,
});

// Exports
export const lambdaFunctionName = lambda.function.name;
export const lambdaFunctionArn = lambda.function.arn;
export const rawEmailsBucketName = storage.bucket.bucket;
export const outboundQueueUrl = queues.outboundQueue.url;
export const defaultQueueUrl = queues.defaultQueue.url;
export const dlqUrl = queues.dlq.url;
export const idempotencyTableName = database.idempotencyTable.name;

// Export app queue URLs
export const appQueueUrls = pulumi.output(
    Object.entries(queues.appQueues).reduce(
        (acc, [appName, queue]) => {
            acc[appName] = queue.url;
            return acc;
        },
        {} as Record<string, pulumi.Output<string>>
    )
);

// Export app queue names for easy reference
export const appQueueNames = Object.keys(queues.appQueues);

// Summary
export const summary = pulumi.interpolate`
Mailflow Infrastructure Deployed Successfully!

Environment: ${environment}
Apps: ${appQueueNames.join(", ")}
Domains: ${domains.join(", ")}

Lambda Function: ${lambda.function.name}
Raw Emails Bucket: ${storage.bucket.bucket}
Outbound Queue: ${queues.outboundQueue.name}

To send test email:
  aws ses send-email --from test@${domains[0]} --destination ToAddresses=_${appQueueNames[0]}@${domains[0]} --message "Subject={Data=Test},Body={Text={Data=Hello}}"

To check app queue:
  aws sqs receive-message --queue-url <queue-url>
`;
