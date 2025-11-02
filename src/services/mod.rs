/// AWS service clients and infrastructure services
pub mod attachments;
pub mod config;
pub mod idempotency;
pub mod metrics;
pub mod rate_limiter;
pub mod s3;
pub mod security;
pub mod ses;
pub mod sqs;

// Re-export service traits
pub use config::ConfigProvider;
pub use idempotency::IdempotencyService;
pub use metrics::MetricsService;
pub use rate_limiter::RateLimiter;
pub use s3::StorageService;
pub use ses::EmailSender;
pub use sqs::QueueService;
