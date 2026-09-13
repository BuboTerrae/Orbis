use super::{ChatMessage, LlmProvider, TokenStream, ToolSpec, openai::OpenAiProvider};
use anyhow::Result;
use async_trait::async_trait;

pub struct DeepSeekProvider {
    inner: OpenAiProvider,
}

impl DeepSeekProvider {
    pub fn new(api_key: String, model: String) -> Self {
        let base_url = "https://api.deepseek.com/v1".to_string();
        Self {
            inner: OpenAiProvider::new(api_key, model, Some(base_url)),
        }
    }
}

#[async_trait]
impl LlmProvider for DeepSeekProvider {
    fn provider_name(&self) -> &'static str {
        "DeepSeek"
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
