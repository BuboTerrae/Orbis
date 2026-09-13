use super::{ChatMessage, LlmProvider, TokenStream, ToolSpec, openai::OpenAiProvider};
use anyhow::Result;
use async_trait::async_trait;

pub struct OpenRouterProvider {
    inner: OpenAiProvider,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        let url = base_url.unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string());
        Self {
            inner: OpenAiProvider::new(api_key, model, Some(url)),
        }
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn provider_name(&self) -> &'static str {
        "OpenRouter"
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }

    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> Result<TokenStream> {
        self.inner.stream_chat(messages, tools).await
    }
}
