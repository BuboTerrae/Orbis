pub mod compact;
pub mod harness;
pub mod runner;
pub mod session;
pub mod system_prompt;
pub mod tools;
pub mod workspace;

use crate::provider::{
    LlmProvider, anthropic::AnthropicProvider, config::Config, config::ProviderType,
    custom::CustomProvider, deepseek::DeepSeekProvider, gemini::GeminiProvider,
    openai::OpenAiProvider, openrouter::OpenRouterProvider,
};
use anyhow::{Result, bail};
use std::sync::Arc;

pub fn build_provider(config: &Config) -> Result<Arc<dyn LlmProvider + Send + Sync>> {
    let provider_type = &config.active_provider;
    let api_key = config
        .get_api_key(provider_type)
        .filter(|k| !k.trim().is_empty());

    let api_key = match api_key {
        Some(k) => k,
        None if matches!(provider_type, ProviderType::Custom) => "ollama".to_string(),
        None => {
            let env_name = match provider_type {
                ProviderType::Gemini => "GEMINI_API_KEY",
                ProviderType::OpenAI => "OPENAI_API_KEY",
                ProviderType::Anthropic => "ANTHROPIC_API_KEY",
                ProviderType::OpenRouter => "OPENROUTER_API_KEY",
                ProviderType::DeepSeek => "DEEPSEEK_API_KEY",
                ProviderType::Custom => "POLYNIA_API_KEY",
            };
            bail!(
                "API Key missing for {}. Set environment variable '{}' or configure it in settings (Alt+K).",
                provider_type,
                env_name
            );
        }
    };

    let model = config.active_model.clone();

    let provider: Arc<dyn LlmProvider + Send + Sync> = match provider_type {
        ProviderType::Gemini => Arc::new(GeminiProvider::new(api_key, model)),
        ProviderType::OpenAI => Arc::new(OpenAiProvider::new(
            api_key,
            model,
            config.openai_base_url.clone(),
        )),
        ProviderType::Anthropic => Arc::new(AnthropicProvider::new(api_key, model)),
        ProviderType::OpenRouter => Arc::new(OpenRouterProvider::new(
            api_key,
            model,
            config.openrouter_base_url.clone(),
        )),
        ProviderType::DeepSeek => Arc::new(DeepSeekProvider::new(api_key, model)),
        ProviderType::Custom => {
            let base = config
                .custom_base_url
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "http://127.0.0.1:11434/v1".to_string());
            Arc::new(CustomProvider::new(api_key, model, base))
        }
    };

    Ok(provider)
}
