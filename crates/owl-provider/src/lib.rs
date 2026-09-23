mod error;
mod mock;

use std::pin::Pin;

use futures_core::Stream;

pub use error::ProviderError;
pub use mock::MockProvider;

use owl_types::{ContextCapture, Target};

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub prompt: Option<String>,
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
}

pub type ProviderStream =
    Pin<Box<dyn Stream<Item = Result<ProviderOutput, ProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub image_input: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderOutput {
    TextDelta(String),
}

pub trait Provider: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;

    fn stream(&self, request: ProviderRequest) -> ProviderStream;
}
