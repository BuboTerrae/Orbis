use super::sse::SseParser;
use super::{
    ChatMessage, LlmProvider, StreamChunk, TokenStream, TokenUsage, ToolCall, ToolSpec,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub struct OpenAiProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        }
    }
}

#[derive(Default)]
struct ToolCallAccumulator {
    calls: Vec<PartialToolCall>,
}

#[derive(Default, Clone)]
struct PartialToolCall {
    id: String,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    fn apply(&mut self, delta: &Value) {
        let index = delta["index"].as_u64().unwrap_or(0) as usize;
        while self.calls.len() <= index {
            self.calls.push(PartialToolCall::default());
        }
        let slot = &mut self.calls[index];
        if let Some(id) = delta["id"].as_str()
            && !id.is_empty()
        {
            slot.id = id.to_string();
        }
        if let Some(name) = delta["function"]["name"].as_str() {
            slot.name.push_str(name);
        }
        if let Some(args) = delta["function"]["arguments"].as_str() {
            slot.arguments.push_str(args);
        }
    }

    fn finish(self) -> Vec<ToolCall> {
        self.calls
            .into_iter()
            .enumerate()
            .filter(|(_, p)| !p.name.is_empty())
            .map(|(i, p)| {
                let arguments = serde_json::from_str(&p.arguments).unwrap_or_else(|_| json!({}));
                ToolCall::new(
                    if p.id.is_empty() {
                        format!("call_{i}")
                    } else {
                        p.id
                    },
                    p.name,
                    arguments,
                )
            })
            .collect()
    }
}

fn emit_finished_tools(
    tx: &tokio::sync::mpsc::UnboundedSender<StreamChunk>,
    acc: ToolCallAccumulator,
) {
    for tc in acc.finish() {
        let _ = tx.send(StreamChunk::ToolCallDelta(tc));
    }
}

pub(crate) fn parse_openai_usage(val: &Value) -> Option<TokenUsage> {
    let usage = val.get("usage")?;
    if !usage.is_object() {
        return None;
    }
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .or_else(|| usage.get("input_tokens").and_then(Value::as_u64))
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .or_else(|| usage.get("output_tokens").and_then(Value::as_u64))
        .unwrap_or(0);
    let cached_tokens = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .or_else(|| usage.get("cached_tokens").and_then(Value::as_u64))
        .unwrap_or(0);
    if input_tokens == 0 && output_tokens == 0 && cached_tokens == 0 {
        return None;
    }
    Some(TokenUsage {
        input_tokens,
        output_tokens,
        cached_tokens,
    })
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn provider_name(&self) -> &'static str {
        "OpenAI"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> Result<TokenStream> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let formatted_messages: Vec<Value> = messages
            .iter()
            .map(|msg| {
                let role_str = match msg.role {
                    super::Role::System => "system",
                    super::Role::User => "user",
                    super::Role::Assistant => "assistant",
                    super::Role::Tool => "tool",
                };
                let mut obj = json!({
                    "role": role_str,
                    "content": if msg.content.is_empty() && msg.tool_calls.is_some() {
                        Value::Null
                    } else {
                        Value::String(msg.content.clone())
                    },
                });

                if let Some(ref name) = msg.name {
                    obj["name"] = json!(name);
                }

                if let Some(ref tool_calls) = msg.tool_calls {
                    let calls: Vec<_> = tool_calls
                        .iter()
                        .map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": if tc.arguments.is_string() {
                                        tc.arguments.as_str().unwrap_or("{}").to_string()
                                    } else {
                                        tc.arguments.to_string()
                                    }
                                }
                            })
                        })
                        .collect();
                    obj["tool_calls"] = json!(calls);
                }

                if let Some(ref tool_call_id) = msg.tool_call_id {
                    obj["tool_call_id"] = json!(tool_call_id);
                }

                obj
            })
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": formatted_messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        if !tools.is_empty() {
            let tools_json: Vec<_> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools_json);
            body["tool_choice"] = json!("auto");
        }

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send OpenAI API request")?;

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI-compatible API error: {}", err_text);
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut stream = res.bytes_stream();
            let mut sse = SseParser::new();
            let mut acc = ToolCallAccumulator::default();
            let mut last_usage = TokenUsage::default();

            let finish = |tx: &tokio::sync::mpsc::UnboundedSender<StreamChunk>,
                          acc: ToolCallAccumulator,
                          usage: &TokenUsage| {
                emit_finished_tools(tx, acc);
                if !usage.is_zero() {
                    let _ = tx.send(StreamChunk::Usage(usage.clone()));
                }
                let _ = tx.send(StreamChunk::Done);
            };

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        for data in sse.push(&bytes) {
                            if data == "[DONE]" {
                                finish(&tx, std::mem::take(&mut acc), &last_usage);
                                return;
                            }

                            let Ok(val) = serde_json::from_str::<Value>(&data) else {
                                continue;
                            };

                            if let Some(err) = val["error"]["message"].as_str() {
                                let _ = tx.send(StreamChunk::Error(err.to_string()));
                                return;
                            }

                            if let Some(usage) = parse_openai_usage(&val) {
                                last_usage = usage;
                            }

                            let Some(choices) = val["choices"].as_array() else {
                                continue;
                            };
                            let Some(first) = choices.first() else {
                                continue;
                            };

                            if let Some(content) = first["delta"]["content"].as_str()
                                && !content.is_empty()
                            {
                                let _ = tx.send(StreamChunk::Token(content.to_string()));
                            }

                            if let Some(tool_calls) = first["delta"]["tool_calls"].as_array() {
                                for tc in tool_calls {
                                    acc.apply(tc);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamChunk::Error(e.to_string()));
                        return;
                    }
                }
            }
            finish(&tx, acc, &last_usage);
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolCallAccumulator, parse_openai_usage};
    use serde_json::json;

    #[test]
    fn accumulates_split_tool_call_deltas() {
        let mut acc = ToolCallAccumulator::default();
        acc.apply(&json!({"index": 0, "id": "call_abc", "function": {"name": "read_file", "arguments": ""}}));
        acc.apply(&json!({"index": 0, "function": {"arguments": "{\"path\":"}}));
        acc.apply(&json!({"index": 0, "function": {"arguments": "\"src/main.rs\"}"}}));
        let calls = acc.finish();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_abc");
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].arguments["path"], "src/main.rs");
    }

    #[test]
    fn usage_chunk_with_empty_choices_is_parsed() {
        let usage = parse_openai_usage(&json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 40,
                "prompt_tokens_details": { "cached_tokens": 16 }
            }
        }))
        .unwrap();
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.output_tokens, 40);
        assert_eq!(usage.cached_tokens, 16);
        assert_eq!(usage.total(), 160);
    }
}
