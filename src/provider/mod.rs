pub mod anthropic;
pub mod config;
pub mod custom;
pub mod deepseek;
pub mod gemini;
pub mod openai;
pub mod openrouter;
pub mod sse;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tools(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            name: None,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    pub fn tool_response(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            name: Some(name.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    /// Gemini 2.5/3 thought signatures must be echoed on subsequent turns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

impl ToolCall {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
            thought_signature: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
}

impl TokenUsage {
    /// Billed tokens for the turn. Cached tokens are a subset of input.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    pub fn is_zero(&self) -> bool {
        self.input_tokens == 0 && self.output_tokens == 0 && self.cached_tokens == 0
    }

    pub fn add_assign(&mut self, other: &TokenUsage) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cached_tokens += other.cached_tokens;
    }
}

/// Rough token estimate when a provider omits usage metadata (~4 chars / token).
pub fn estimate_tokens(text: &str) -> u64 {
    let chars = text.chars().count() as u64;
    if chars == 0 {
        0
    } else {
        (chars + 3) / 4
    }
}

pub fn estimate_message_tokens(messages: &[ChatMessage]) -> u64 {
    messages
        .iter()
        .map(|m| {
            let mut n = estimate_tokens(&m.content);
            if let Some(calls) = &m.tool_calls {
                for tc in calls {
                    n += estimate_tokens(&tc.name);
                    n += estimate_tokens(&tc.arguments.to_string());
                }
            }
            n
        })
        .sum()
}

#[derive(Debug, Clone)]
pub enum StreamChunk {
    Token(String),
    ToolCallDelta(ToolCall),
    Done,
    Error(String),
    Usage(TokenUsage),
}

pub type TokenStream = Pin<Box<dyn Stream<Item = StreamChunk> + Send>>;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    #[allow(dead_code)]
    fn provider_name(&self) -> &'static str;
    #[allow(dead_code)]
    fn model_name(&self) -> &str;
    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> Result<TokenStream>;
}
