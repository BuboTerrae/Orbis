use crate::agent::harness::{ensure_tool_call_id, is_blocked_in_readonly, should_auto_allow};
use crate::app::{ActiveModal, App, FocusedPanel};
use crate::provider::config::ProviderType;
use crate::provider::{ChatMessage, StreamChunk, ToolCall};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::VecDeque;
use tokio::sync::mpsc::UnboundedSender;

pub enum AppEvent {
    LlmToken {
        generation_id: u64,
        token: String,
    },
    LlmToolCall {
        generation_id: u64,
        tool_call: ToolCall,
    },
    LlmDone {
        generation_id: u64,
    },
    LlmError {
        generation_id: u64,
        error: String,
    },
    ToolResult {
        generation_id: u64,
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
        result: String,
    },
}

pub fn handle_key_event(
    app: &mut App,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        app.should_quit = true;
        return Ok(());
    }

    match app.active_modal.clone() {
        ActiveModal::None => handle_main_keys(app, key, tx)?,
        ActiveModal::ProviderPicker { selected_index } => {
            handle_provider_picker_keys(app, selected_index, key)?
        }
        ActiveModal::Settings {
            selected_tab,
            selected_field,
            input_buffer,
        } => handle_settings_keys(app, selected_tab, selected_field, input_buffer, key)?,
        ActiveModal::ToolApproval {
            tool_call_id,
            tool_name,
            arguments,
        } => handle_tool_approval_keys(app, tool_call_id, tool_name, arguments, key, tx)?,
        ActiveModal::SessionPicker { selected_index } => {
            handle_session_picker_keys(app, selected_index, key)?
        }
        ActiveModal::Help => {
            if key.code == KeyCode::Esc || key.code == KeyCode::Enter {
                app.active_modal = ActiveModal::None;
            }
        }
    }
    Ok(())
}

fn handle_slash_command(app: &mut App, line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('/') {
        return false;
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    let cmd = parts.first().copied().unwrap_or("");

    match cmd {
        "/provider" => {
            if let Some(target) = parts.get(1) {
                let target_lower = target.to_lowercase();
                let matched_provider = ProviderType::all().into_iter().find(|p| {
                    p.to_string().to_lowercase().contains(&target_lower)
                        || match p {
                            ProviderType::Gemini => "gemini",
                            ProviderType::OpenAI => "openai",
                            ProviderType::Anthropic => "anthropic",
                            ProviderType::OpenRouter => "openrouter",
                            ProviderType::DeepSeek => "deepseek",
                            ProviderType::Custom => "custom",
                        } == target_lower
                });

                if let Some(p) = matched_provider {
                    app.config.active_provider = p.clone();
                    app.config.active_model = p.default_model().to_string();
                    let _ = app.config.save();
                    app.status_message = Some(format!("Switched provider to {}", p));
                } else {
                    app.status_message = Some(format!(
                        "Unknown provider '{}'. Available: gemini, openai, anthropic, openrouter, deepseek, custom",
                        target
                    ));
                }
            } else {
                app.active_modal = ActiveModal::ProviderPicker { selected_index: 0 };
            }
            true
        }
        "/model" => {
            if let Some(model_name) = parts.get(1) {
                app.config.active_model = model_name.to_string();
                let _ = app.config.save();
                app.status_message = Some(format!("Switched model to {}", model_name));
            } else {
                app.status_message = Some(format!(
                    "Usage: /model <model_name>. Current: {}",
                    app.config.active_model
                ));
            }
            true
        }
        "/settings" | "/config" | "/keys" | "/key" => {
            app.active_modal = ActiveModal::Settings {
                selected_tab: 0,
                selected_field: 0,
                input_buffer: String::new(),
            };
            true
        }
        "/mode" => {
            if let Some(target_mode) = parts.get(1) {
                match target_mode.to_lowercase().as_str() {
                    "agent" => app.mode = crate::app::AgentMode::Agent,
                    "chat" | "direct" => app.mode = crate::app::AgentMode::DirectChat,
                    _ => {}
                }
            } else {
                app.mode = match app.mode {
                    crate::app::AgentMode::Agent => crate::app::AgentMode::DirectChat,
                    crate::app::AgentMode::DirectChat => crate::app::AgentMode::Agent,
                };
            }
            app.status_message = Some(format!("Active mode: {}", app.mode));
            true
        }
        "/permission" | "/security" | "/sec" => {
            if let Some(target) = parts.get(1) {
                match target.to_lowercase().as_str() {
                    "ask" => app.permission_mode = crate::app::PermissionMode::Ask,
                    "auto" | "autoapprove" => {
                        app.permission_mode = crate::app::PermissionMode::AutoApprove
                    }
                    "readonly" | "read" => {
                        app.permission_mode = crate::app::PermissionMode::ReadOnly
                    }
                    _ => {}
                }
            } else {
                app.permission_mode = match app.permission_mode {
                    crate::app::PermissionMode::Ask => crate::app::PermissionMode::AutoApprove,
                    crate::app::PermissionMode::AutoApprove => crate::app::PermissionMode::ReadOnly,
                    crate::app::PermissionMode::ReadOnly => crate::app::PermissionMode::Ask,
                };
            }
            app.status_message = Some(format!("Permission mode: {}", app.permission_mode));
            true
        }
        "/ping" => {
            let provider_res = crate::agent::build_provider(&app.config);
            match provider_res {
                Ok(p) => {
                    app.status_message = Some(format!(
                        "Provider {} [{}] ready.",
                        p.provider_name(),
                        p.model_name()
                    ));
                }
                Err(e) => {
                    app.status_message = Some(format!("Ping error: {}", e));
                }
            }
            true
        }
        "/tree" => {
            match crate::agent::tools::file_ops::list_dir(&serde_json::json!({ "path": "." })) {
                Ok(res) => app.status_message = Some(format!("Workspace:\n{}", res)),
                Err(e) => app.status_message = Some(format!("Tree error: {e}")),
            }
            true
        }
        "/new" => {
            app.clear_conversation();
            app.status_message = Some("Started a new session.".to_string());
            true
        }
        "/sessions" | "/session" => {
            app.active_modal = ActiveModal::SessionPicker { selected_index: 0 };
            true
        }
        "/resume" => {
            if let Some(id) = parts.get(1) {
                match crate::agent::session::load(id) {
                    Ok(session) => app.apply_session(session),
                    Err(e) => app.status_message = Some(format!("Resume failed: {e}")),
                }
            } else {
                app.active_modal = ActiveModal::SessionPicker { selected_index: 0 };
            }
            true
        }
        "/compact" => {
            let before = app.messages.len();
            app.messages = crate::agent::compact::compact_messages(&app.messages, 3, 1_200);
            app.status_message = Some(format!(
                "Compacted conversation ({} → {} messages).",
                before,
                app.messages.len()
            ));
            app.persist_session();
            true
        }
        "/endpoint" => {
            if let Some(url) = parts.get(1) {
                app.config.openai_base_url = Some((*url).to_string());
                let _ = app.config.save();
                app.status_message = Some(format!(
                    "OpenAI-compatible base URL set to {}. Switch provider to OpenAI to use it.",
                    url
                ));
            } else {
                app.status_message = Some(format!(
                    "Usage: /endpoint <base_url>. Current: {}",
                    app.config
                        .openai_base_url
                        .as_deref()
                        .unwrap_or("https://api.openai.com/v1")
                ));
            }
            true
        }
        "/clear" => {
            app.clear_conversation();
            app.status_message = Some("Cleared conversation history.".to_string());
            true
        }
        "/status" => {
            app.status_message = Some(format!(
                "Provider: {} | Model: {} | Mode: {} | Perms: {}",
                app.config.active_provider, app.config.active_model, app.mode, app.permission_mode
            ));
            true
        }
        "/debug" => {
            app.status_message = Some(format!(
                "Session ID: {:?} | Focus: {:?}",
                app.session_id, app.focused_panel
            ));
            true
        }
        _ => {
            app.status_message = Some(format!(
                "Unknown slash command '{}'. Type /help for available commands.",
                cmd
            ));
            true
        }
    }
}

fn handle_main_keys(app: &mut App, key: KeyEvent, tx: &UnboundedSender<AppEvent>) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Char('p') | KeyCode::Char('P') => {
                app.active_modal = ActiveModal::ProviderPicker { selected_index: 0 };
                return Ok(());
            }
            KeyCode::Char('k') | KeyCode::Char('K') => {
                app.active_modal = ActiveModal::Settings {
                    selected_tab: 0,
                    selected_field: 0,
                    input_buffer: String::new(),
                };
                return Ok(());
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                app.permission_mode = match app.permission_mode {
                    crate::app::PermissionMode::Ask => crate::app::PermissionMode::AutoApprove,
                    crate::app::PermissionMode::AutoApprove => crate::app::PermissionMode::ReadOnly,
                    crate::app::PermissionMode::ReadOnly => crate::app::PermissionMode::Ask,
                };
                app.status_message = Some(format!("Permission mode: {}", app.permission_mode));
                return Ok(());
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                app.mode = match app.mode {
                    crate::app::AgentMode::Agent => crate::app::AgentMode::DirectChat,
                    crate::app::AgentMode::DirectChat => crate::app::AgentMode::Agent,
                };
                app.status_message = Some(format!("Mode: {}", app.mode));
                return Ok(());
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                app.clear_conversation();
                return Ok(());
            }
            KeyCode::Char('h') | KeyCode::Char('H') => {
                app.active_modal = ActiveModal::Help;
                return Ok(());
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                app.active_modal = ActiveModal::SessionPicker { selected_index: 0 };
                return Ok(());
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                app.should_quit = true;
                return Ok(());
            }
            KeyCode::Enter => {
                if app.focused_panel == FocusedPanel::Input {
                    app.input_buffer.push('\n');
                }
                return Ok(());
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            if app.is_streaming || !app.tool_queue.is_empty() {
                app.cancel_generation();
            } else if !app.input_buffer.is_empty() {
                app.input_buffer.clear();
            }
        }
        KeyCode::Tab => {
            app.focused_panel = match app.focused_panel {
                FocusedPanel::Input => FocusedPanel::Chat,
                FocusedPanel::Chat => FocusedPanel::Input,
            };
        }
        KeyCode::Up if app.focused_panel == FocusedPanel::Chat => {
            app.follow_chat = false;
            app.chat_scroll = app.chat_scroll.saturating_sub(1);
        }
        KeyCode::Down if app.focused_panel == FocusedPanel::Chat => {
            app.chat_scroll += 1;
        }
        KeyCode::Up if app.focused_panel == FocusedPanel::Input => {
            recall_history(app, -1);
        }
        KeyCode::Down if app.focused_panel == FocusedPanel::Input => {
            recall_history(app, 1);
        }
        KeyCode::PageUp if app.focused_panel == FocusedPanel::Chat => {
            app.follow_chat = false;
            app.chat_scroll = app.chat_scroll.saturating_sub(10);
        }
        KeyCode::PageDown if app.focused_panel == FocusedPanel::Chat => {
            app.chat_scroll += 10;
        }
        KeyCode::Enter if app.focused_panel == FocusedPanel::Input => {
            if !app.input_buffer.trim().is_empty() && !app.is_streaming {
                let prompt = std::mem::take(&mut app.input_buffer);

                if !handle_slash_command(app, &prompt) {
                    app.input_history.push(prompt.clone());
                    app.history_index = None;
                    app.messages.push(ChatMessage::user(&prompt));
                    app.follow_chat = true;
                    app.status_message = None;
                    trigger_llm_generation(app, tx.clone());
                }
            }
        }
        KeyCode::Char(c) if app.focused_panel == FocusedPanel::Input => {
            app.input_buffer.push(c);
        }
        KeyCode::Backspace if app.focused_panel == FocusedPanel::Input => {
            app.input_buffer.pop();
        }
        _ => {}
    }

    Ok(())
}

fn recall_history(app: &mut App, delta: i32) {
    if app.input_history.is_empty() {
        return;
    }
    let last = app.input_history.len() - 1;
    let next = match app.history_index {
        None if delta < 0 => last,
        None => return,
        Some(i) if delta < 0 => i.saturating_sub(1),
        Some(i) => {
            if i >= last {
                app.history_index = None;
                app.input_buffer.clear();
                return;
            }
            i + 1
        }
    };
    app.history_index = Some(next);
    app.input_buffer = app.input_history[next].clone();
}

fn handle_provider_picker_keys(app: &mut App, selected_index: usize, key: KeyEvent) -> Result<()> {
    let providers = ProviderType::all();
    match key.code {
        KeyCode::Esc => {
            app.active_modal = ActiveModal::None;
        }
        KeyCode::Up => {
            let next_idx = if selected_index == 0 {
                providers.len() - 1
            } else {
                selected_index - 1
            };
            app.active_modal = ActiveModal::ProviderPicker {
                selected_index: next_idx,
            };
        }
        KeyCode::Down => {
            let next_idx = (selected_index + 1) % providers.len();
            app.active_modal = ActiveModal::ProviderPicker {
                selected_index: next_idx,
            };
        }
        KeyCode::Enter => {
            if let Some(selected_provider) = providers.get(selected_index) {
                app.config.active_provider = selected_provider.clone();
                app.config.active_model = selected_provider.default_model().to_string();
                let _ = app.config.save();
            }
            app.active_modal = ActiveModal::None;
        }
        _ => {}
    }
    Ok(())
}

// const KEY_FIELD_COUNT: usize = 6;

fn handle_settings_keys(
    app: &mut App,
    selected_tab: usize,
    selected_field: usize,
    mut input_buffer: String,
    key: KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.active_modal = ActiveModal::None;
        }
        KeyCode::Tab => {
            let next_tab = (selected_tab + 1) % 2;
            app.active_modal = ActiveModal::Settings {
                selected_tab: next_tab,
                selected_field: 0,
                input_buffer: String::new(),
            };
        }
        KeyCode::Up => {
            if selected_tab == 0 { // Keys tab
                let next_field = if selected_field == 0 { 5 } else { selected_field - 1 };
                app.active_modal = ActiveModal::Settings {
                    selected_tab,
                    selected_field: next_field,
                    input_buffer: String::new(),
                };
            }
        }
        KeyCode::Down => {
            if selected_tab == 0 {
                let next_field = (selected_field + 1) % 6;
                app.active_modal = ActiveModal::Settings {
                    selected_tab,
                    selected_field: next_field,
                    input_buffer: String::new(),
                };
            }
        }
        KeyCode::Char(c) => {
            input_buffer.push(c);
            app.active_modal = ActiveModal::Settings {
                selected_tab,
                selected_field,
                input_buffer,
            };
        }
        KeyCode::Backspace => {
            input_buffer.pop();
            app.active_modal = ActiveModal::Settings {
                selected_tab,
                selected_field,
                input_buffer,
            };
        }
        KeyCode::Enter => {
            if !input_buffer.is_empty() {
                if selected_tab == 0 {
                    match selected_field {
                        0 => app.config.gemini_api_key = Some(input_buffer),
                        1 => app.config.openai_api_key = Some(input_buffer),
                        2 => app.config.anthropic_api_key = Some(input_buffer),
                        3 => app.config.openrouter_api_key = Some(input_buffer),
                        4 => app.config.deepseek_api_key = Some(input_buffer),
                        5 => app.config.custom_api_key = Some(input_buffer),
                        _ => {}
                    }
                    app.config.save()?;
                }
            } else if selected_tab == 1 { // Models tab
                let providers = crate::provider::config::ProviderType::all();
                if let Some(p) = providers.get(selected_field) {
                    app.config.active_provider = p.clone();
                    app.config.active_model = p.default_model().to_string();
                    app.config.save()?;
                }
            }
            app.active_modal = ActiveModal::None;
        }
        _ => {}
    }
    Ok(())
}

fn handle_tool_approval_keys(
    app: &mut App,
    tool_call_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) -> Result<()> {
    let generation_id = app.generation.current_id();
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            app.active_modal = ActiveModal::None;
            app.tools_in_flight += 1;
            spawn_tool(
                generation_id,
                tool_call_id,
                tool_name,
                arguments,
                app.config.clone(),
                tx.clone(),
            );
        }
        KeyCode::Char('a') | KeyCode::Char('A') => {
            app.active_modal = ActiveModal::None;
            app.always_allowed_tools.insert(tool_name.clone());
            app.tools_in_flight += 1;
            spawn_tool(
                generation_id,
                tool_call_id,
                tool_name,
                arguments,
                app.config.clone(),
                tx.clone(),
            );
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.active_modal = ActiveModal::None;
            let _ = tx.send(AppEvent::ToolResult {
                generation_id,
                tool_call_id,
                tool_name,
                arguments,
                result: "User denied execution of this tool call.".to_string(),
            });
        }
        _ => {}
    }
    Ok(())
}

fn spawn_tool(
    generation_id: u64,
    tool_call_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    config: crate::provider::config::Config,
    tx: UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        let result =
            crate::agent::runner::execute_named_tool(&config, &tool_name, &arguments, 0, false)
                .await;
        let _ = tx.send(AppEvent::ToolResult {
            generation_id,
            tool_call_id,
            tool_name,
            arguments,
            result,
        });
    });
}

pub fn trigger_llm_generation(app: &mut App, tx: UnboundedSender<AppEvent>) {
    app.prune_messages_if_needed();
    app.is_streaming = true;
    app.streaming_buffer.clear();
    app.pending_tool_calls.clear();
    app.tools_in_flight = 0;
    app.follow_chat = true;

    let generation_id = app.generation.start();
    let gen_id = app.generation.id.clone();

    let provider_res = crate::agent::build_provider(&app.config);
    let provider = match provider_res {
        Ok(p) => p,
        Err(e) => {
            app.is_streaming = false;
            let _ = tx.send(AppEvent::LlmError {
                generation_id,
                error: e.to_string(),
            });
            return;
        }
    };

    let messages = app.messages.clone();
    let tools = match app.mode {
        crate::app::AgentMode::Agent => crate::agent::tools::ToolRegistry::all_specs(),
        crate::app::AgentMode::DirectChat => vec![],
    };

    tokio::spawn(async move {
        let stale = || gen_id.load(std::sync::atomic::Ordering::SeqCst) != generation_id;
        match provider.stream_chat(&messages, &tools).await {
            Ok(mut stream) => {
                use tokio_stream::StreamExt;
                while let Some(chunk) = stream.next().await {
                    if stale() {
                        return;
                    }
                    match chunk {
                        StreamChunk::Token(token) => {
                            let _ = tx.send(AppEvent::LlmToken {
                                generation_id,
                                token,
                            });
                        }
                        StreamChunk::ToolCallDelta(tc) => {
                            let _ = tx.send(AppEvent::LlmToolCall {
                                generation_id,
                                tool_call: tc,
                            });
                        }
                        StreamChunk::Error(error) => {
                            let _ = tx.send(AppEvent::LlmError {
                                generation_id,
                                error,
                            });
                            return;
                        }
                        StreamChunk::Done => {
                            let _ = tx.send(AppEvent::LlmDone { generation_id });
                            return;
                        }
                    }
                }
                if !stale() {
                    let _ = tx.send(AppEvent::LlmDone { generation_id });
                }
            }
            Err(e) => {
                if !stale() {
                    let _ = tx.send(AppEvent::LlmError {
                        generation_id,
                        error: e.to_string(),
                    });
                }
            }
        }
    });
}

pub fn apply_event(app: &mut App, event: AppEvent, tx: &UnboundedSender<AppEvent>) {
    match event {
        AppEvent::LlmToken {
            generation_id,
            token,
        } => {
            if !app.generation.is_current(generation_id) {
                return;
            }
            app.streaming_buffer.push_str(&token);
        }
        AppEvent::LlmToolCall {
            generation_id,
            mut tool_call,
        } => {
            if !app.generation.is_current(generation_id) {
                return;
            }
            ensure_tool_call_id(&mut tool_call, app.pending_tool_calls.len());
            app.pending_tool_calls.push(tool_call);
        }
        AppEvent::LlmDone { generation_id } => {
            if !app.generation.is_current(generation_id) {
                return;
            }
            let text = std::mem::take(&mut app.streaming_buffer);
            let tool_calls = std::mem::take(&mut app.pending_tool_calls);
            if tool_calls.is_empty() {
                app.is_streaming = false;
                if !text.is_empty() {
                    app.messages.push(ChatMessage::assistant(text));
                }
                app.persist_session();
            } else {
                app.messages
                    .push(ChatMessage::assistant_with_tools(text, tool_calls.clone()));
                app.tool_queue = VecDeque::from(tool_calls);
                dispatch_next_tool(app, tx);
            }
        }
        AppEvent::LlmError {
            generation_id,
            error,
        } => {
            if !app.generation.is_current(generation_id) {
                return;
            }
            app.is_streaming = false;
            app.status_message = Some(error);
        }
        AppEvent::ToolResult {
            generation_id,
            tool_call_id,
            tool_name,
            arguments,
            result,
        } => {
            if !app.generation.is_current(generation_id) {
                return;
            }
            if tool_name == "update_todos" {
                if let Ok(todos) = crate::agent::tools::todo_ops::parse_todos(&arguments) {
                    app.todos = todos;
                }
            }
            app.messages
                .push(ChatMessage::tool_response(tool_call_id, tool_name, result));
            app.tools_in_flight = app.tools_in_flight.saturating_sub(1);
            app.persist_session();
            if app.tools_in_flight > 0 {
                return;
            }
            if app.tool_queue.is_empty() {
                trigger_llm_generation(app, tx.clone());
            } else {
                dispatch_next_tool(app, tx);
            }
        }
    }
}

fn dispatch_next_tool(app: &mut App, tx: &UnboundedSender<AppEvent>) {
    let generation_id = app.generation.current_id();

    loop {
        let Some(tc) = app.tool_queue.front() else {
            if app.tools_in_flight == 0 {
                trigger_llm_generation(app, tx.clone());
            }
            return;
        };

        if is_blocked_in_readonly(app, &tc.name) {
            let tc = app.tool_queue.pop_front().unwrap();
            app.messages.push(ChatMessage::tool_response(
                tc.id,
                tc.name,
                "Tool execution blocked: permission mode is Read-only.",
            ));
            continue;
        }

        if should_auto_allow(app, &tc.name) {
            let tc = app.tool_queue.pop_front().unwrap();
            app.tools_in_flight += 1;
            spawn_tool(
                generation_id,
                tc.id,
                tc.name,
                tc.arguments,
                app.config.clone(),
                tx.clone(),
            );
            continue;
        }

        if app.tools_in_flight > 0 {
            return;
        }

        let tc = app.tool_queue.pop_front().unwrap();
        app.active_modal = ActiveModal::ToolApproval {
            tool_call_id: tc.id,
            tool_name: tc.name,
            arguments: tc.arguments,
        };
        return;
    }
}

fn handle_session_picker_keys(app: &mut App, selected_index: usize, key: KeyEvent) -> Result<()> {
    let sessions = crate::agent::session::list().unwrap_or_default();
    match key.code {
        KeyCode::Esc => {
            app.active_modal = ActiveModal::None;
        }
        KeyCode::Up => {
            let next = if sessions.is_empty() {
                0
            } else if selected_index == 0 {
                sessions.len() - 1
            } else {
                selected_index - 1
            };
            app.active_modal = ActiveModal::SessionPicker {
                selected_index: next,
            };
        }
        KeyCode::Down => {
            let next = if sessions.is_empty() {
                0
            } else {
                (selected_index + 1) % sessions.len()
            };
            app.active_modal = ActiveModal::SessionPicker {
                selected_index: next,
            };
        }
        KeyCode::Enter => {
            if let Some(meta) = sessions.get(selected_index) {
                match crate::agent::session::load(&meta.id) {
                    Ok(session) => app.apply_session(session),
                    Err(e) => {
                        app.status_message = Some(format!("Failed to load session: {e}"));
                        app.active_modal = ActiveModal::None;
                    }
                }
            } else {
                app.active_modal = ActiveModal::None;
            }
        }
        _ => {}
    }
    Ok(())
}
