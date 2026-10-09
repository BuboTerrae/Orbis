use super::sse::SseParser;
use super::{
    ChatMessage, LlmProvider, StreamChunk, TokenStream, TokenUsage, ToolCall, ToolSpec,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{Map, Value, json};
use tokio_stream::wrappers::UnboundedReceiverStream;

pub struct GeminiProvider {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }
}

/// Convert Orbis messages into Gemini `systemInstruction` + `contents`.
/// Consecutive same-role turns are merged (Gemini rejects them), and tool
/// results are sent as `user` `functionResponse` parts.
pub(crate) fn format_request(messages: &[ChatMessage]) -> (Option<Value>, Vec<Value>) {
    let mut system = String::new();
    let mut contents: Vec<(String, Vec<Value>)> = Vec::new();

    let push = |contents: &mut Vec<(String, Vec<Value>)>, role: &str, parts: Vec<Value>| {
        if parts.is_empty() {
            return;
        }
        if let Some((last_role, last_parts)) = contents.last_mut()
            && last_role == role
        {
            last_parts.extend(parts);
            return;
        }
        contents.push((role.to_string(), parts));
    };

    for msg in messages {
        match msg.role {
            super::Role::System => {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(&msg.content);
            }
            super::Role::User => {
                push(&mut contents, "user", vec![json!({"text": msg.content})]);
            }
            super::Role::Assistant => {
                let mut parts = Vec::new();
                if !msg.content.is_empty() {
                    parts.push(json!({"text": msg.content}));
                }
                if let Some(ref tool_calls) = msg.tool_calls {
                    for tc in tool_calls {
                        let mut part = json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": tc.arguments,
                            }
                        });
                        if let Some(sig) = &tc.thought_signature
                            && !sig.is_empty()
                        {
                            part["thoughtSignature"] = json!(sig);
                        }
                        parts.push(part);
                    }
                }
                if parts.is_empty() {
                    parts.push(json!({"text": ""}));
                }
                push(&mut contents, "model", parts);
            }
            super::Role::Tool => {
                let name = msg.name.as_deref().unwrap_or("");
                let response = match serde_json::from_str::<Value>(&msg.content) {
                    Ok(v) => v,
                    Err(_) => json!({ "content": msg.content }),
                };
                push(
                    &mut contents,
                    "user",
                    vec![json!({
                        "functionResponse": {
                            "name": name,
                            "response": if response.is_object() {
                                response
                            } else {
                                json!({ "content": response })
                            }
                        }
                    })],
                );
            }
        }
    }

    if contents.first().map(|(r, _)| r.as_str()) == Some("model") {
        contents.insert(
            0,
            (
                "user".to_string(),
                vec![json!({"text": "Begin."})],
            ),
        );
    }

    let contents = contents
        .into_iter()
        .map(|(role, parts)| json!({ "role": role, "parts": parts }))
        .collect();

    let system_instruction = if system.trim().is_empty() {
        None
    } else {
        Some(json!({ "parts": [{ "text": system }] }))
    };

    (system_instruction, contents)
}

fn sanitize_schema(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if matches!(
                    k.as_str(),
                    "$schema"
                        | "$id"
                        | "additionalProperties"
                        | "additional_properties"
                        | "examples"
                        | "default"
                        | "exclusiveMinimum"
                        | "exclusiveMaximum"
                ) {
                    continue;
                }
                out.insert(k.clone(), sanitize_schema(v));
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sanitize_schema).collect()),
        other => other.clone(),
    }
}

pub(crate) fn format_tools(tools: &[ToolSpec]) -> Value {
    let decls: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "parameters": sanitize_schema(&t.parameters),
            })
        })
        .collect();
    json!([{ "functionDeclarations": decls }])
}

fn parse_usage(val: &Value) -> Option<TokenUsage> {
    let usage = val.get("usageMetadata")?.as_object()?;
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        + usage
            .get("thoughtsTokenCount")
            .and_then(Value::as_u64)
            .unwrap_or(0);
    let cached_tokens = usage
        .get("cachedContentTokenCount")
        .and_then(Value::as_u64)
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

fn parse_function_call(part: &Value, index: usize) -> Option<ToolCall> {
    let fc = part
        .get("functionCall")
        .or_else(|| part.get("function_call"))?
        .as_object()?;
    let name = fc.get("name").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        return None;
    }
    let args = fc
        .get("args")
        .cloned()
        .or_else(|| fc.get("arguments").cloned())
        .unwrap_or_else(|| json!({}));
    let arguments = if args.is_string() {
        serde_json::from_str(args.as_str().unwrap_or("{}")).unwrap_or(json!({}))
    } else {
        args
    };
    let thought_signature = part
        .get("thoughtSignature")
        .or_else(|| part.get("thought_signature"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some(ToolCall {
        id: format!("call_{index}"),
        name: name.to_string(),
        arguments,
        thought_signature,
    })
}

fn is_terminal_finish(reason: &str) -> bool {
    matches!(
        reason,
        "STOP"
            | "MAX_TOKENS"
            | "SAFETY"
            | "RECITATION"
            | "LANGUAGE"
            | "BLOCKLIST"
            | "PROHIBITED_CONTENT"
            | "SPII"
            | "MALFORMED_FUNCTION_CALL"
            | "OTHER"
    )
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn provider_name(&self) -> &'static str {
        "Google Gemini"
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
    ) -> Result<TokenStream> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.model, self.api_key
        );

        let (system_instruction, contents) = format_request(messages);

        let mut body = json!({
            "contents": contents,
            "generationConfig": {
                "temperature": 0.7,
                "maxOutputTokens": 8192,
            }
        });

        if let Some(sys) = system_instruction {
            body["systemInstruction"] = sys;
        }

        if !tools.is_empty() {
            body["tools"] = format_tools(tools);
            body["toolConfig"] = json!({"functionCallingConfig": {"mode": "AUTO"}});
        }

        let res = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send Gemini API request")?;

        if !res.status().is_success() {
            let err_text = res.text().await.unwrap_or_default();
            anyhow::bail!("Gemini API error: {}", err_text);
        }

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut stream = res.bytes_stream();
            let mut sse = SseParser::new();
            let mut pending_function_calls = Vec::new();
            let mut last_usage = TokenUsage::default();
            let mut call_index = 0usize;
            let mut finished = false;

            let emit_finish = |tx: &tokio::sync::mpsc::UnboundedSender<StreamChunk>,
                               pending: &mut Vec<ToolCall>,
                               usage: &TokenUsage| {
                for tc in pending.drain(..) {
                    let _ = tx.send(StreamChunk::ToolCallDelta(tc));
                }
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
                                emit_finish(&tx, &mut pending_function_calls, &last_usage);
                                return;
                            }

                            let Ok(val) = serde_json::from_str::<Value>(&data) else {
                                continue;
                            };

                            if let Some(err) = val.get("error") {
                                let msg = err
                                    .get("message")
                                    .and_then(Value::as_str)
                                    .or_else(|| err.as_str())
                                    .unwrap_or("Gemini stream error");
                                let _ = tx.send(StreamChunk::Error(msg.to_string()));
                                return;
                            }

                            if let Some(feedback) = val.get("promptFeedback")
                                && let Some(reason) =
                                    feedback.get("blockReason").and_then(Value::as_str)
                                && reason != "BLOCK_REASON_UNSPECIFIED"
                            {
                                let _ = tx.send(StreamChunk::Error(format!(
                                    "Gemini blocked the prompt ({reason})"
                                )));
                                return;
                            }

                            if let Some(u) = parse_usage(&val) {
                                last_usage = u;
                            }

                            if let Some(candidates) = val["candidates"].as_array() {
                                for candidate in candidates {
                                    if let Some(parts) =
                                        candidate["content"]["parts"].as_array()
                                    {
                                        for part in parts {
                                            if part.get("thought").and_then(Value::as_bool)
                                                == Some(true)
                                            {
                                                continue;
                                            }
                                            if let Some(text) = part["text"].as_str()
                                                && !text.is_empty()
                                            {
                                                let _ = tx.send(StreamChunk::Token(
                                                    text.to_string(),
                                                ));
                                            }
                                            if let Some(tc) =
                                                parse_function_call(part, call_index)
                                            {
                                                call_index += 1;
                                                pending_function_calls.push(tc);
                                            }
                                        }
                                    }

                                    if let Some(reason) =
                                        candidate["finishReason"].as_str()
                                        && is_terminal_finish(reason)
                                    {
                                        if reason == "SAFETY" || reason == "RECITATION" {
                                            let _ = tx.send(StreamChunk::Error(format!(
                                                "Gemini stopped ({reason})"
                                            )));
                                            return;
                                        }
                                        emit_finish(
                                            &tx,
                                            &mut pending_function_calls,
                                            &last_usage,
                                        );
                                        finished = true;
                                        return;
                                    }
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
            if !finished {
                emit_finish(&tx, &mut pending_function_calls, &last_usage);
            }
        });

        Ok(Box::pin(UnboundedReceiverStream::new(rx)))
    }
}

#[cfg(test)]
mod tests {
    use super::{format_request, format_tools, parse_function_call, parse_usage, sanitize_schema};
    use crate::provider::{ChatMessage, ToolCall, ToolSpec};
    use serde_json::json;

    #[test]
    fn system_prompt_is_not_a_user_turn() {
        let msgs = vec![
            ChatMessage::system("be helpful"),
            ChatMessage::user("hi"),
        ];
        let (sys, contents) = format_request(&msgs);
        assert!(sys.is_some());
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
    }

    #[test]
    fn merges_consecutive_user_roles_from_tool_results() {
        let msgs = vec![
            ChatMessage::user("look at main"),
            ChatMessage::assistant_with_tools(
                "",
                vec![
                    ToolCall::new("1", "read_file", json!({"path": "a.rs"})),
                    ToolCall::new("2", "read_file", json!({"path": "b.rs"})),
                ],
            ),
            ChatMessage::tool_response("1", "read_file", "fn a() {}"),
            ChatMessage::tool_response("2", "read_file", "fn b() {}"),
        ];
        let (_, contents) = format_request(&msgs);
        assert_eq!(contents.len(), 3);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[2]["role"], "user");
        assert_eq!(contents[2]["parts"].as_array().unwrap().len(), 2);
        assert!(contents[2]["parts"][0].get("functionResponse").is_some());
    }

    #[test]
    fn tools_are_a_single_function_declarations_array() {
        let tools = vec![
            ToolSpec {
                name: "a".into(),
                description: "A".into(),
                parameters: json!({"type": "object", "additionalProperties": false, "properties": {}}),
            },
            ToolSpec {
                name: "b".into(),
                description: "B".into(),
                parameters: json!({"type": "object", "properties": {}}),
            },
        ];
        let formatted = format_tools(&tools);
        assert_eq!(formatted.as_array().unwrap().len(), 1);
        let decls = formatted[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 2);
        assert!(decls[0]["parameters"].get("additionalProperties").is_none());
    }

    #[test]
    fn parses_usage_and_function_call_casings() {
        let usage = parse_usage(&json!({
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 4,
                "thoughtsTokenCount": 2,
                "cachedContentTokenCount": 3
            }
        }))
        .unwrap();
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 6);
        assert_eq!(usage.cached_tokens, 3);

        let tc = parse_function_call(
            &json!({
                "functionCall": {"name": "read_file", "args": {"path": "x"}},
                "thoughtSignature": "sig"
            }),
            0,
        )
        .unwrap();
        assert_eq!(tc.name, "read_file");
        assert_eq!(tc.thought_signature.as_deref(), Some("sig"));
    }

    #[test]
    fn sanitize_drops_unsupported_keys() {
        let v = sanitize_schema(&json!({
            "type": "object",
            "additionalProperties": false,
            "$schema": "http://json-schema.org/draft-07/schema#",
            "properties": { "x": { "type": "string", "default": "a" } }
        }));
        assert!(v.get("additionalProperties").is_none());
        assert!(v.get("$schema").is_none());
        assert!(v["properties"]["x"].get("default").is_none());
    }
}
