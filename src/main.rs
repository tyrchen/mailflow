use lambda_runtime::{Error, run, service_fn};
use mailflow::handler;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing subscriber for structured logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .json()
        .init();

    info!("Starting Mailflow Lambda function");

    // Run the Lambda runtime
    run(service_fn(handler)).await
}
