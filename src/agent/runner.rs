use crate::agent::compact;
use crate::agent::harness::{ensure_tool_call_id, is_read_tool, truncate_tool_output};
use crate::agent::system_prompt;
use crate::agent::tools::ToolRegistry;
use crate::provider::config::Config;
use crate::provider::{ChatMessage, LlmProvider, StreamChunk, TokenUsage, ToolCall, ToolSpec};
use anyhow::{Context, Result};
use tokio_stream::StreamExt;

// fn assert_send<T: Send>(_: T) {}

pub struct RunOptions {
    pub max_turns: usize,
    pub readonly: bool,
    pub depth: u8,
    pub print_tokens: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            max_turns: 32,
            readonly: false,
            depth: 0,
            print_tokens: false,
        }
    }
}

pub async fn collect_completion(
    provider: &(dyn LlmProvider + Send + Sync),
    messages: &[ChatMessage],
    tools: &[ToolSpec],
) -> Result<(String, Vec<ToolCall>, TokenUsage)> {
    let mut stream = provider.stream_chat(messages, tools).await?;
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    let mut usage = TokenUsage::default();
    while let Some(chunk) = stream.next().await {
        match chunk {
            StreamChunk::Token(tok) => text.push_str(&tok),
            StreamChunk::ToolCallDelta(mut tc) => {
                ensure_tool_call_id(&mut tc, tool_calls.len());
                if !tc.name.is_empty() {
                    tool_calls.push(tc);
                }
            }
            StreamChunk::Usage(u) => usage = u,
            StreamChunk::Error(err) => anyhow::bail!(err),
            StreamChunk::Done => break,
        }
    }
    Ok((text, tool_calls, usage))
}

pub fn tools_for_depth(depth: u8) -> Vec<ToolSpec> {
    let mut specs = ToolRegistry::all_specs();
    if depth >= 1 {
        specs.retain(|s| s.name != "spawn_subagent");
    }
    specs
}

pub fn execute_named_tool<'a>(
    config: &'a Config,
    name: &'a str,
    arguments: &'a serde_json::Value,
    depth: u8,
    readonly: bool,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
    let config = config.clone();
    let name = name.to_string();
    let arguments = arguments.clone();
    Box::pin(async move {
        if readonly && !is_read_tool(&name) && name != "spawn_subagent" {
            return format!("Tool '{name}' blocked: sub-agent/run is read-only.");
        }
        if name == "spawn_subagent" {
            if depth >= 1 {
                return "Nested spawn_subagent is not allowed (depth limit).".to_string();
            }
            return match run_subagent_from_args(&config, &arguments, depth + 1).await {
                Ok(out) => out,
                Err(e) => format!("Sub-agent failed: {e:#}"),
            };
        }
        match ToolRegistry::execute_tool(&name, &arguments).await {
            Ok(out) => out,
            Err(e) => format!("Tool Execution Error: {e:?}"),
        }
    })
}

pub async fn run_loop(
    config: &Config,
    mut messages: Vec<ChatMessage>,
    options: RunOptions,
) -> Result<String> {
    let provider = crate::agent::build_provider(config)?;
    let tools = tools_for_depth(options.depth);
    let mut last_text = String::new();
    let mut session_usage = TokenUsage::default();

    for turn in 0..options.max_turns {
        if compact::should_compact(&messages) {
            messages = compact::compact_messages(&messages, 4, 2_000);
        }
        let (text, tool_calls, usage) =
            collect_completion(provider.as_ref(), &messages, &tools).await?;
        let usage = if usage.is_zero() {
            TokenUsage {
                input_tokens: crate::provider::estimate_message_tokens(&messages),
                output_tokens: crate::provider::estimate_tokens(&text),
                cached_tokens: 0,
            }
        } else {
            usage
        };
        session_usage.add_assign(&usage);
        if options.print_tokens && !text.is_empty() {
            print!("{text}");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        last_text = text.clone();

        if tool_calls.is_empty() {
            if !text.is_empty() {
                messages.push(ChatMessage::assistant(text));
            }
            if options.print_tokens && !session_usage.is_zero() {
                eprintln!(
                    "\n[tokens] in={} out={} cached={} total={}",
                    session_usage.input_tokens,
                    session_usage.output_tokens,
                    session_usage.cached_tokens,
                    session_usage.total()
                );
            }
            return Ok(last_text);
        }

        messages.push(ChatMessage::assistant_with_tools(text, tool_calls.clone()));

        let mut pending = Vec::new();
        let mut rest = Vec::new();
        for tc in tool_calls {
            if options.readonly && !is_read_tool(&tc.name) && tc.name != "spawn_subagent" {
                messages.push(ChatMessage::tool_response(
                    tc.id,
                    tc.name,
                    "Tool execution blocked: read-only run.",
                ));
            } else if is_read_tool(&tc.name) {
                pending.push(tc);
            } else {
                rest.push(tc);
            }
        }

        // Just run sequentially for now to resolve Send issues
        for tc in pending {
            let result = execute_named_tool(
                config,
                &tc.name,
                &tc.arguments,
                options.depth,
                options.readonly,
            )
            .await;
            if options.print_tokens {
                eprintln!("[{}]", tc.name);
            }
            messages.push(ChatMessage::tool_response(tc.id, tc.name, result));
        }

        for tc in rest {
            if options.print_tokens {
                eprintln!("[{}]", tc.name);
            }
            let result = execute_named_tool(
                config,
                &tc.name,
                &tc.arguments,
                options.depth,
                options.readonly,
            )
            .await;
            messages.push(ChatMessage::tool_response(tc.id, tc.name, result));
        }

        let _ = turn;
    }

    if options.print_tokens && !session_usage.is_zero() {
        eprintln!(
            "\n[tokens] in={} out={} cached={} total={}",
            session_usage.input_tokens,
            session_usage.output_tokens,
            session_usage.cached_tokens,
            session_usage.total()
        );
    }

    Ok(format!(
        "{last_text}\n\n[stopped after {} tool-loop turns]",
        options.max_turns
    ))
}

pub async fn run_prompt(config: &Config, prompt: &str, options: RunOptions) -> Result<String> {
    let messages = vec![
        ChatMessage::system(system_prompt::get_system_prompt()),
        ChatMessage::user(prompt),
    ];
    run_loop(config, messages, options).await
}

pub async fn run_subagent_from_args(
    config: &Config,
    args: &serde_json::Value,
    depth: u8,
) -> Result<String> {
    let prompt = args["prompt"]
        .as_str()
        .context("spawn_subagent requires 'prompt'")?;
    let readonly = args["readonly"].as_bool().unwrap_or(true);
    let mut options = RunOptions {
        max_turns: args["max_turns"].as_u64().unwrap_or(12).clamp(1, 24) as usize,
        readonly,
        depth,
        print_tokens: false,
    };
    // Sub-agents stay cheaper/shorter by default.
    if options.max_turns > 16 {
        options.max_turns = 16;
    }
    let text = run_prompt(config, prompt, options).await?;
    Ok(truncate_tool_output(format!(
        "Sub-agent (readonly={readonly}, depth={depth}) finished:\n{text}"
    )))
}

pub fn parse_provider_flag(name: &str) -> Option<crate::provider::config::ProviderType> {
    use crate::provider::config::ProviderType;
    match name.to_lowercase().as_str() {
        "gemini" | "google" => Some(ProviderType::Gemini),
        "openai" => Some(ProviderType::OpenAI),
        "anthropic" | "claude" => Some(ProviderType::Anthropic),
        "openrouter" => Some(ProviderType::OpenRouter),
        "deepseek" => Some(ProviderType::DeepSeek),
        "custom" => Some(ProviderType::Custom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::tools_for_depth;

    #[test]
    fn nested_depth_strips_spawn_tool() {
        let top = tools_for_depth(0);
        let nested = tools_for_depth(1);
        assert!(top.iter().any(|t| t.name == "spawn_subagent"));
        assert!(nested.iter().all(|t| t.name != "spawn_subagent"));
    }
}
