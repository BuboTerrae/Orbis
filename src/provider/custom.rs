use super::{ChatMessage, LlmProvider, TokenStream, ToolSpec, openai::OpenAiProvider};
use anyhow::Result;
use async_trait::async_trait;

pub struct CustomProvider {
    inner: OpenAiProvider,
}

impl CustomProvider {
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self {
            inner: OpenAiProvider::new(api_key, model, Some(base_url)),
        }
    }
}

#[async_trait]
impl LlmProvider for CustomProvider {
    fn provider_name(&self) -> &'static str {
        "Custom"
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
