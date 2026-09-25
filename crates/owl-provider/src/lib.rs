mod error;
mod instructions;
mod mock;
mod openai;

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use futures_core::Stream;
use serde::{Deserialize, Serialize};

pub use error::ProviderError;
pub use mock::MockProvider;
pub use openai::{OpenAiConfig, OpenAiProvider};

use owl_types::{ContextCapture, Target};

pub const OPENAI_PROVIDER_ID: &str = "openai";

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub prompt: Option<String>,
    pub target: Option<Target>,
    pub context: Option<ContextCapture>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider: ProviderId,
    pub model: String,
}

impl ModelSelection {
    pub fn new(provider: ProviderId, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }
}

impl fmt::Display for ModelSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} · {}", self.model, self.provider)
    }
}

pub type ProviderStream =
    Pin<Box<dyn Stream<Item = Result<ProviderOutput, ProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderOutput {
    TextDelta(String),
}

pub trait Provider: Send + Sync {
    fn available_models(&self) -> Vec<String>;

    fn stream(&self, model: &str, request: ProviderRequest) -> ProviderStream;
}

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<HashMap<ProviderId, Arc<dyn Provider>>>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_provider(
        &self,
        id: ProviderId,
        provider: Arc<dyn Provider>,
    ) -> Option<Arc<dyn Provider>> {
        self.providers
            .write()
            .expect("provider registry lock poisoned")
            .insert(id, provider)
    }

    pub fn remove_provider(&self, id: &ProviderId) -> Option<Arc<dyn Provider>> {
        self.providers
            .write()
            .expect("provider registry lock poisoned")
            .remove(id)
    }

    pub fn resolve(&self, selection: &ModelSelection) -> Option<Arc<dyn Provider>> {
        self.providers
            .read()
            .expect("provider registry lock poisoned")
            .get(&selection.provider)
            .cloned()
    }

    pub fn available_models(&self) -> Vec<ModelSelection> {
        let providers = self
            .providers
            .read()
            .expect("provider registry lock poisoned")
            .iter()
            .map(|(id, provider)| (id.clone(), Arc::clone(provider)))
            .collect::<Vec<_>>();
        let mut models = providers
            .into_iter()
            .flat_map(|(id, provider)| {
                provider
                    .available_models()
                    .into_iter()
                    .map(move |model| ModelSelection::new(id.clone(), model))
            })
            .collect::<Vec<_>>();
        models.sort();
        models
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn registry_lists_and_resolves_provider_models() {
        let registry = ProviderRegistry::new();
        let shared_registry = registry.clone();
        let provider_id = ProviderId::new("mock");
        registry.add_provider(
            provider_id.clone(),
            Arc::new(MockProvider::new(Duration::ZERO)),
        );

        let models = shared_registry.available_models();

        assert_eq!(
            models,
            vec![ModelSelection::new(provider_id.clone(), "mock")]
        );
        assert!(shared_registry.resolve(&models[0]).is_some());
        registry.remove_provider(&provider_id);
        assert!(shared_registry.available_models().is_empty());
        assert!(shared_registry.resolve(&models[0]).is_none());
    }

    #[test]
    fn model_selection_has_a_dropdown_label() {
        let selection = ModelSelection::new(ProviderId::new("openai"), "gpt-test");

        assert_eq!(selection.to_string(), "gpt-test · openai");
    }
}
