import * as pulumi from "@pulumi/pulumi";
import * as aws from "@pulumi/aws";
import * as fs from "fs";
import * as path from "path";

export interface ApiGatewayConfig {
    apiLambda: aws.lambda.Function;
    environment: string;
}

export function createApiGateway(config: ApiGatewayConfig) {
    const { apiLambda, environment } = config;

    // 1. Create API Gateway REST API
    const api = new aws.apigateway.RestApi(`mailflow-api-${environment}`, {
        name: `mailflow-api-${environment}`,
        description: "Mailflow Dashboard API",
        endpointConfiguration: {
            types: "REGIONAL",
        },
    });

    // 2. Create proxy resource for all API routes
    const proxyResource = new aws.apigateway.Resource(`api-proxy-${environment}`, {
        restApi: api.id,
        parentId: api.rootResourceId,
        pathPart: "{proxy+}",
    });

    // 3. Create ANY method (no Lambda authorizer, JWT validation happens in Lambda)
    const proxyMethod = new aws.apigateway.Method(`api-proxy-method-${environment}`, {
        restApi: api.id,
        resourceId: proxyResource.id,
        httpMethod: "ANY",
        authorization: "NONE", // JWT validation done in Lambda
    });

    // 4. Create OPTIONS method for CORS preflight
    const optionsMethod = new aws.apigateway.Method(`api-options-method-${environment}`, {
        restApi: api.id,
        resourceId: proxyResource.id,
        httpMethod: "OPTIONS",
        authorization: "NONE",
    });

    // 5. OPTIONS method response
    const optionsMethodResponse = new aws.apigateway.MethodResponse(
        `api-options-method-response-${environment}`,
        {
            restApi: api.id,
            resourceId: proxyResource.id,
            httpMethod: optionsMethod.httpMethod,
            statusCode: "200",
            responseParameters: {
                "method.response.header.Access-Control-Allow-Headers": true,
                "method.response.header.Access-Control-Allow-Methods": true,
                "method.response.header.Access-Control-Allow-Origin": true,
            },
        }
    );

    // 6. OPTIONS integration (mock)
    const optionsIntegration = new aws.apigateway.Integration(
        `api-options-integration-${environment}`,
        {
            restApi: api.id,
            resourceId: proxyResource.id,
            httpMethod: optionsMethod.httpMethod,
            type: "MOCK",
            requestTemplates: {
                "application/json": '{"statusCode": 200}',
            },
        }
    );

    // 7. OPTIONS integration response
    const optionsIntegrationResponse = new aws.apigateway.IntegrationResponse(
        `api-options-integration-response-${environment}`,
        {
            restApi: api.id,
            resourceId: proxyResource.id,
            httpMethod: optionsMethod.httpMethod,
            statusCode: optionsMethodResponse.statusCode,
            responseParameters: {
                "method.response.header.Access-Control-Allow-Headers":
                    "'Content-Type,X-Amz-Date,Authorization,X-Api-Key,X-Amz-Security-Token'",
                "method.response.header.Access-Control-Allow-Methods": "'GET,OPTIONS,POST,PUT,DELETE'",
                "method.response.header.Access-Control-Allow-Origin": "'*'",
            },
        },
        { dependsOn: [optionsMethodResponse] }
    );

    // 8. Lambda integration for proxy method
    const integration = new aws.apigateway.Integration(`api-integration-${environment}`, {
        restApi: api.id,
        resourceId: proxyResource.id,
        httpMethod: proxyMethod.httpMethod,
        integrationHttpMethod: "POST",
        type: "AWS_PROXY",
        uri: apiLambda.invokeArn,
    });

    // 9. Lambda permission for API Gateway
    const permission = new aws.lambda.Permission(`api-lambda-permission-${environment}`, {
        action: "lambda:InvokeFunction",
        function: apiLambda.name,
        principal: "apigateway.amazonaws.com",
        sourceArn: pulumi.interpolate`${api.executionArn}/*/*`,
    });

    // 10. Deployment
    const deployment = new aws.apigateway.Deployment(
        `api-deployment-${environment}`,
        {
            restApi: api.id,
            stageName: "", // Stage name set in stage resource
        },
        {
            dependsOn: [integration, optionsIntegration],
        }
    );

    // 11. Stage
    const stage = new aws.apigateway.Stage(`api-stage-${environment}`, {
        restApi: api.id,
        deployment: deployment.id,
        stageName: "v1",
        description: `Mailflow API ${environment} stage`,
    });

    return {
        api,
        deployment,
        stage,
        apiUrl: pulumi.interpolate`${api.id}.execute-api.${aws.config.region}.amazonaws.com/${stage.stageName}`,
    };
}
