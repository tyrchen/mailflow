import * as aws from "@pulumi/aws";
import * as pulumi from "@pulumi/pulumi";

export interface SesConfig {
    lambdaFunction: aws.lambda.Function;
    rawEmailsBucket: aws.s3.Bucket;
    domains: string[];
    environment: string;
}

export function createSesConfiguration(config: SesConfig) {
    const { lambdaFunction, rawEmailsBucket, domains, environment } = config;

    // Grant SES permission to invoke Lambda
    const sesLambdaPermission = new aws.lambda.Permission(`mailflow-ses-invoke-${environment}`, {
        action: "lambda:InvokeFunction",
        function: lambdaFunction.name,
        principal: "ses.amazonaws.com",
    });

    // SES Receipt Rule Set
    const ruleSet = new aws.ses.ReceiptRuleSet(`mailflow-rules-${environment}`, {
        ruleSetName: `mailflow-rules-${environment}`,
    });

    // Activate the rule set
    const activeRuleSet = new aws.ses.ActiveReceiptRuleSet(`mailflow-active-${environment}`, {
        ruleSetName: ruleSet.ruleSetName,
    });

    // SES Receipt Rule
    const receiptRule = new aws.ses.ReceiptRule(`mailflow-inbound-${environment}`, {
        name: `mailflow-inbound-${environment}`,
        ruleSetName: ruleSet.ruleSetName,
        enabled: true,
        recipients: domains,
        scanEnabled: true,
        s3Actions: [
            {
                bucketName: rawEmailsBucket.bucket,
                position: 1,
            },
        ],
        lambdaActions: [
            {
                functionArn: lambdaFunction.arn,
                position: 2,
                invocationType: "Event",
            },
        ],
    }, { dependsOn: [sesLambdaPermission] });

    return {
        ruleSet,
        activeRuleSet,
        receiptRule,
        sesLambdaPermission,
    };
}
