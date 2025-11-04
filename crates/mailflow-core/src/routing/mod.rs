/// Email routing modules
pub mod engine;
pub mod resolver;
pub mod rules;

pub use engine::Router;
pub use rules::{RouteDestination, extract_app_name};
