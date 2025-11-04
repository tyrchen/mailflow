use lambda_http::{Error, Request, run, service_fn};
use mailflow_api::ApiContext;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .json()
        .init();

    info!("Starting Mailflow API Lambda function");

    // Initialize API context
    let ctx = ApiContext::new().await?;

    // Run the Lambda runtime with our handler
    run(service_fn(|event: Request| {
        let ctx = ctx.clone();
        async move { mailflow_api::handler(ctx, event).await }
    }))
    .await
}
