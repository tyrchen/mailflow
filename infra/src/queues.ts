import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export interface QueueResources {
    appQueues: Record<string, aws.sqs.Queue>;
    outboundQueue: aws.sqs.Queue;
    defaultQueue: aws.sqs.Queue;
    dlq: aws.sqs.Queue;
}

export function createQueues(environment: string, apps: string[]): QueueResources {
    // Dead letter queue
    const dlq = new aws.sqs.Queue(`mailflow-dlq-${environment}`, {
        name: `mailflow-dlq-${environment}`,
        messageRetentionSeconds: 1209600, // 14 days
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Create app-specific queues dynamically
    const appQueues: Record<string, aws.sqs.Queue> = {};

    for (const appName of apps) {
        const queue = new aws.sqs.Queue(`mailflow-${appName}-${environment}`, {
            name: `mailflow-${appName}-${environment}`,
            visibilityTimeoutSeconds: 300,
            messageRetentionSeconds: 1209600, // 14 days
            receiveWaitTimeSeconds: 20, // Long polling
            redrivePolicy: pulumi.jsonStringify({
                deadLetterTargetArn: dlq.arn,
                maxReceiveCount: 5,
            }),
            tags: {
                Environment: environment,
                Service: "mailflow",
                App: appName,
            },
        });

        appQueues[appName] = queue;
    }

    // Outbound queue (shared by all apps)
    const outboundQueue = new aws.sqs.Queue(`mailflow-outbound-${environment}`, {
        name: `mailflow-outbound-${environment}`,
        visibilityTimeoutSeconds: 300,
        messageRetentionSeconds: 1209600,
        receiveWaitTimeSeconds: 20,
        redrivePolicy: pulumi.jsonStringify({
            deadLetterTargetArn: dlq.arn,
            maxReceiveCount: 5,
        }),
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Default queue (for emails not matching any app pattern)
    const defaultQueue = new aws.sqs.Queue(`mailflow-default-${environment}`, {
        name: `mailflow-default-${environment}`,
        visibilityTimeoutSeconds: 300,
        messageRetentionSeconds: 1209600,
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    return {
        appQueues,
        outboundQueue,
        defaultQueue,
        dlq,
    };
}
