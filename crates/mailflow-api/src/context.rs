/// API Context - shared state for all API handlers
use crate::auth::JwtValidator;
use lambda_http::Error;
use std::sync::Arc;

/// API Context contains shared resources for API handlers
#[derive(Clone)]
pub struct ApiContext {
    /// AWS configuration
    pub aws_config: aws_config::SdkConfig,

    /// S3 client
    pub s3_client: aws_sdk_s3::Client,

    /// SQS client
    pub sqs_client: aws_sdk_sqs::Client,

    /// CloudWatch client
    pub cloudwatch_client: aws_sdk_cloudwatch::Client,

    /// CloudWatch Logs client
    pub logs_client: aws_sdk_cloudwatchlogs::Client,

    /// DynamoDB client
    pub dynamodb_client: aws_sdk_dynamodb::Client,

    /// SES client (for test emails)
    pub ses_client: aws_sdk_ses::Client,

    /// JWT validator
    pub jwt_validator: Arc<JwtValidator>,

    /// Expected JWT issuer (from environment)
    pub jwt_issuer: String,
}

impl ApiContext {
    /// Create a new API context
    pub async fn new() -> Result<Arc<Self>, Error> {
        // Load AWS config
        let aws_config = aws_config::load_from_env().await;

        // Create AWS clients
        let s3_client = aws_sdk_s3::Client::new(&aws_config);
        let sqs_client = aws_sdk_sqs::Client::new(&aws_config);
        let cloudwatch_client = aws_sdk_cloudwatch::Client::new(&aws_config);
        let logs_client = aws_sdk_cloudwatchlogs::Client::new(&aws_config);
        let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);
        let ses_client = aws_sdk_ses::Client::new(&aws_config);

        // Load JWKS from environment
        let jwks_json =
            std::env::var("JWKS_JSON").map_err(|_| "JWKS_JSON environment variable not set")?;

        // Load JWT issuer from environment
        let jwt_issuer =
            std::env::var("JWT_ISSUER").map_err(|_| "JWT_ISSUER environment variable not set")?;

        // Create JWT validator
        let jwt_validator = Arc::new(JwtValidator::new(&jwks_json)?);

        Ok(Arc::new(Self {
            aws_config,
            s3_client,
            sqs_client,
            cloudwatch_client,
            logs_client,
            dynamodb_client,
            ses_client,
            jwt_validator,
            jwt_issuer,
        }))
    }
}
