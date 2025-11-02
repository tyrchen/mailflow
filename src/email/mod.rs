pub mod attachment;
pub mod composer;
pub mod mime;
/// Email processing modules
pub mod parser;

pub use composer::EmailComposer;
pub use parser::EmailParser;
