use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderType {
    Gemini,
    OpenAI,
    Anthropic,
    OpenRouter,
    DeepSeek,
    Custom,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Gemini => write!(f, "Google Gemini"),
            ProviderType::OpenAI => write!(f, "OpenAI"),
            ProviderType::Anthropic => write!(f, "Anthropic"),
            ProviderType::OpenRouter => write!(f, "OpenRouter"),
            ProviderType::DeepSeek => write!(f, "DeepSeek"),
            ProviderType::Custom => write!(f, "Custom (OpenAI-compatible)"),
        }
    }
}

impl ProviderType {
    pub fn all() -> Vec<ProviderType> {
        vec![
            ProviderType::Gemini,
            ProviderType::OpenAI,
            ProviderType::Anthropic,
            ProviderType::OpenRouter,
            ProviderType::DeepSeek,
            ProviderType::Custom,
        ]
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            ProviderType::Gemini => "gemini-3.1-flash-lite",
            ProviderType::OpenAI => "gpt-4o",
            ProviderType::Anthropic => "claude-3-5-sonnet-20241022",
            ProviderType::OpenRouter => "google/gemini-3.1-flash-lite",
            ProviderType::DeepSeek => "deepseek-chat",
            ProviderType::Custom => "llama3.2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub active_provider: ProviderType,
    pub active_model: String,

    pub gemini_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub openrouter_api_key: Option<String>,
    #[serde(default)]
    pub deepseek_api_key: Option<String>,
    #[serde(default)]
    pub custom_api_key: Option<String>,

    pub openai_base_url: Option<String>,
    pub openrouter_base_url: Option<String>,
    #[serde(default)]
    pub custom_base_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_provider: ProviderType::Gemini,
            active_model: "gemini-3.1-flash-lite".to_string(),
            gemini_api_key: None,
            openai_api_key: None,
            anthropic_api_key: None,
            openrouter_api_key: None,
            deepseek_api_key: None,
            custom_api_key: None,
            openai_base_url: None,
            openrouter_base_url: None,
            custom_base_url: None,
        }
    }
}

impl Config {
    pub fn config_file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("polynia").join("config.toml"))
    }

    pub fn load() -> Self {
        dotenvy::dotenv().ok();
        if let Some(path) = Self::config_file_path() {
            if path.exists() {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(cfg) = toml::from_str::<Config>(&content) {
                        return cfg;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::config_file_path() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }

    pub fn get_api_key(&self, provider: &ProviderType) -> Option<String> {
        match provider {
            ProviderType::Gemini => env::var("GEMINI_API_KEY")
                .or_else(|_| env::var("GOOGLE_API_KEY"))
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.gemini_api_key.clone()),

            ProviderType::OpenAI => env::var("OPENAI_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.openai_api_key.clone()),

            ProviderType::Anthropic => env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.anthropic_api_key.clone()),

            ProviderType::OpenRouter => env::var("OPENROUTER_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.openrouter_api_key.clone()),
            ProviderType::DeepSeek => env::var("DEEPSEEK_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.deepseek_api_key.clone()),
            ProviderType::Custom => env::var("POLYNIA_API_KEY")
                .or_else(|_| env::var("CUSTOM_API_KEY"))
                .or_else(|_| env::var("OPENAI_API_KEY"))
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| self.custom_api_key.clone())
                .or_else(|| self.openai_api_key.clone()),
        }
    }

    pub fn is_key_from_env(&self, provider: &ProviderType) -> bool {
        match provider {
            ProviderType::Gemini => {
                env::var("GEMINI_API_KEY").is_ok() || env::var("GOOGLE_API_KEY").is_ok()
            }
            ProviderType::OpenAI => env::var("OPENAI_API_KEY").is_ok(),
            ProviderType::Anthropic => env::var("ANTHROPIC_API_KEY").is_ok(),
            ProviderType::OpenRouter => env::var("OPENROUTER_API_KEY").is_ok(),
            ProviderType::DeepSeek => env::var("DEEPSEEK_API_KEY").is_ok(),
            ProviderType::Custom => {
                env::var("POLYNIA_API_KEY").is_ok()
                    || env::var("CUSTOM_API_KEY").is_ok()
                    || env::var("OPENAI_API_KEY").is_ok()
            }
        }
    }
}
