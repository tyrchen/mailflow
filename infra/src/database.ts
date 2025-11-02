import * as aws from "@pulumi/aws";

export function createDatabaseTables(environment: string) {
    // Idempotency table with TTL
    const idempotencyTable = new aws.dynamodb.Table(`mailflow-idempotency-${environment}`, {
        name: `mailflow-idempotency-${environment}`,
        attributes: [
            {
                name: "correlation_id",
                type: "S",
            },
        ],
        hashKey: "correlation_id",
        billingMode: "PAY_PER_REQUEST",
        ttl: {
            enabled: true,
            attributeName: "ttl",
        },
        tags: {
            Environment: environment,
            Service: "mailflow",
        },
    });

    return {
        idempotencyTable,
    };
}
