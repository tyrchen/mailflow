import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export interface MonitoringConfig {
    lambdaFunction: aws.lambda.Function;
    dlq: aws.sqs.Queue;
    environment: string;
}

export function createMonitoring(config: MonitoringConfig) {
    const { lambdaFunction, dlq, environment } = config;

    // Lambda error alarm
    const lambdaErrorAlarm = new aws.cloudwatch.MetricAlarm(`mailflow-lambda-errors-${environment}`, {
        name: `mailflow-lambda-errors-${environment}`,
        comparisonOperator: "GreaterThanThreshold",
        evaluationPeriods: 1,
        metricName: "Errors",
        namespace: "AWS/Lambda",
        period: 300,
        statistic: "Sum",
        threshold: 10,
        treatMissingData: "notBreaching",
        dimensions: {
            FunctionName: lambdaFunction.name,
        },
        alarmDescription: "Alert when Lambda function has more than 10 errors in 5 minutes",
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // DLQ message alarm
    const dlqAlarm = new aws.cloudwatch.MetricAlarm(`mailflow-dlq-messages-${environment}`, {
        name: `mailflow-dlq-messages-${environment}`,
        comparisonOperator: "GreaterThanThreshold",
        evaluationPeriods: 1,
        metricName: "ApproximateNumberOfMessagesVisible",
        namespace: "AWS/SQS",
        period: 60,
        statistic: "Average",
        threshold: 0,
        treatMissingData: "notBreaching",
        dimensions: {
            QueueName: dlq.name,
        },
        alarmDescription: "Alert when DLQ has any messages",
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    // Lambda duration alarm (detect slow processing)
    const lambdaDurationAlarm = new aws.cloudwatch.MetricAlarm(
        `mailflow-lambda-duration-${environment}`,
        {
            name: `mailflow-lambda-duration-${environment}`,
            comparisonOperator: "GreaterThanThreshold",
            evaluationPeriods: 2,
            metricName: "Duration",
            namespace: "AWS/Lambda",
            period: 300,
            statistic: "Average",
            threshold: 30000, // 30 seconds
            treatMissingData: "notBreaching",
            dimensions: {
                FunctionName: lambdaFunction.name,
            },
            alarmDescription: "Alert when Lambda execution time exceeds 30 seconds average",
            tags: {
                Environment: environment,
                Service: "mailflow",
            },
        }
    );

    return {
        lambdaErrorAlarm,
        dlqAlarm,
        lambdaDurationAlarm,
    };
}
