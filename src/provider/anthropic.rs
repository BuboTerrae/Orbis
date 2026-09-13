use super::{ChatMessage, LlmProvider, StreamChunk, TokenStream, ToolCall, ToolSpec};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Value, json};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }
}

fn format_messages(messages: &[ChatMessage]) -> (String, Vec<Value>) {
    let mut system_prompt = String::new();
    let mut formatted = Vec::new();
    let mut pending_tool_results: Vec<Value> = Vec::new();

    let flush_tool_results = |formatted: &mut Vec<Value>, pending: &mut Vec<Value>| {
        if pending.is_empty() {
            return;
        }
        formatted.push(json!({
            "role": "user",
            "content": std::mem::take(pending),
        }));
    };

    for msg in messages {
        match msg.role {
            super::Role::System => {
                if !system_prompt.is_empty() {
                    system_prompt.push('\n');
                }
                system_prompt.push_str(&msg.content);
            }
            super::Role::User => {
                flush_tool_results(&mut formatted, &mut pending_tool_results);
                formatted.push(json!({
                    "role": "user",
                    "content": msg.content
                }));
            }
            super::Role::Assistant => {
                flush_tool_results(&mut formatted, &mut pending_tool_results);
                let mut content_blocks = Vec::new();
                if !msg.content.is_empty() {
                    content_blocks.push(json!({
                        "type": "text",
                        "text": msg.content
                    }));
                }
                if let Some(ref tool_calls) = msg.tool_calls {
                    for tc in tool_calls {
                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments,
                        }));
                    }
                }
                if content_blocks.is_empty() {
                    content_blocks.push(json!({"type": "text", "text": ""}));
                }
                formatted.push(json!({
                    "role": "assistant",
                    "content": content_blocks
                }));
            }
            super::Role::Tool => {
                pending_tool_results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                    "content": msg.content,
                }));
            }
        }
    }
    flush_tool_results(&mut formatted, &mut pending_tool_results);
    (system_prompt, formatted)
}

#[derive(Default)]
struct AnthropicToolAcc {
    id: String,
    name: String,
    json: String,
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn provider_name(&self) -> &'static str {
        "Anthropic"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> Result<TokenStream> {
        let url = "https://api.anthropic.com/v1/messages";
        let (system_prompt, formatted_messages) = format_messages(messages);

        let mut body = json!({
            "model": self.model,
            "messages": formatted_messages,
            "max_tokens": 8096,
            "stream": true,
        });

        if !system_prompt.is_empty() {
            body["system"] = json!(system_prompt);
        }

        if !tools.is_empty() {
            let tools_json: Vec<_> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            body["tools"] = json!(tools_json);
        }

        let res = self
            .client
            .post(url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send Anthropic API request")?;

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error: {}", err_text);
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut stream = res.bytes_stream();
            let mut buffer = String::new();
            let mut current_tool: Option<AnthropicToolAcc> = None;

            let finish_tool = |tx: &tokio::sync::mpsc::UnboundedSender<StreamChunk>,
                               tool: AnthropicToolAcc| {
                let arguments = serde_json::from_str(&tool.json).unwrap_or(json!({}));
                let _ = tx.send(StreamChunk::ToolCallDelta(ToolCall {
                    id: tool.id,
                    name: tool.name,
                    arguments,
                }));
            };

            while let Some(chunk_res) = stream.next().await {
                match chunk_res {
                    Ok(bytes) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        while let Some(line_end) = buffer.find('\n') {
                            let line = buffer[..line_end].trim().to_string();
                            buffer.drain(..=line_end);

                            if !line.starts_with("data: ") {
                                continue;
                            }
                            let data = line["data: ".len()..].trim();
                            let Ok(val) = serde_json::from_str::<Value>(data) else {
                                continue;
                            };
                            let event_type = val["type"].as_str().unwrap_or("");

                            match event_type {
                                "content_block_start" => {
                                    let block = &val["content_block"];
                                    if block["type"].as_str() == Some("tool_use") {
                                        current_tool = Some(AnthropicToolAcc {
                                            id: block["id"].as_str().unwrap_or("").to_string(),
                                            name: block["name"].as_str().unwrap_or("").to_string(),
                                            json: String::new(),
                                        });
                                    }
                                }
                                "content_block_delta" => {
                                    let delta = &val["delta"];
                                    match delta["type"].as_str().unwrap_or("") {
                                        "text_delta" => {
                                            if let Some(text) = delta["text"].as_str() {
                                                if !text.is_empty() {
                                                    let _ = tx
                                                        .send(StreamChunk::Token(text.to_string()));
                                                }
                                            }
                                        }
                                        "input_json_delta" => {
                                            if let Some(partial) = delta["partial_json"].as_str() {
                                                if let Some(ref mut tool) = current_tool {
                                                    tool.json.push_str(partial);
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                "content_block_stop" => {
                                    if let Some(tool) = current_tool.take() {
                                        finish_tool(&tx, tool);
                                    }
                                }
                                "error" => {
                                    let msg = val["error"]["message"]
                                        .as_str()
                                        .unwrap_or("Anthropic stream error");
                                    let _ = tx.send(StreamChunk::Error(msg.to_string()));
                                    return;
                                }
                                "message_stop" => {
                                    if let Some(tool) = current_tool.take() {
                                        finish_tool(&tx, tool);
                                    }
                                    let _ = tx.send(StreamChunk::Done);
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(StreamChunk::Error(e.to_string()));
                        return;
                    }
                }
            }
            if let Some(tool) = current_tool.take() {
                finish_tool(&tx, tool);
            }
            let _ = tx.send(StreamChunk::Done);
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }
}
