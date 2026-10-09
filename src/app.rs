use crate::agent::harness::GenerationControl;
use crate::agent::session::{self, Session};
use crate::agent::tools::todo_ops::TodoItem;
use crate::provider::{ChatMessage, TokenUsage, ToolCall, config::Config};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    Input,
    Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    Agent,
    DirectChat,
}

impl std::fmt::Display for AgentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentMode::Agent => write!(f, "Agent"),
            AgentMode::DirectChat => write!(f, "Chat"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModal {
    None,
    ProviderPicker {
        selected_index: usize,
    },
    Settings {
        selected_tab: usize,
        selected_field: usize,
        input_buffer: String,
    },
    ToolApproval {
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    SessionPicker {
        selected_index: usize,
    },
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    Ask,
    AutoApprove,
    ReadOnly,
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionMode::Ask => write!(f, "Ask"),
            PermissionMode::AutoApprove => write!(f, "Auto"),
            PermissionMode::ReadOnly => write!(f, "Read-only"),
        }
    }
}

pub struct App {
    pub config: Config,
    pub mode: AgentMode,
    pub permission_mode: PermissionMode,
    pub always_allowed_tools: std::collections::HashSet<String>,
    pub messages: Vec<ChatMessage>,
    pub todos: Vec<TodoItem>,
    pub session_id: String,
    pub input_buffer: String,
    pub focused_panel: FocusedPanel,
    pub active_modal: ActiveModal,
    pub is_streaming: bool,
    pub streaming_buffer: String,
    pub pending_tool_calls: Vec<ToolCall>,
    pub tool_queue: VecDeque<ToolCall>,
    pub tools_in_flight: usize,
    pub generation: GenerationControl,
    pub follow_chat: bool,
    pub status_message: Option<String>,
    pub chat_scroll: usize,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub should_quit: bool,
    pub token_usage: TokenUsage,
    pub session_token_usage: TokenUsage,
    pub pending_turn_usage: Option<TokenUsage>,
    pub slash_completion: Option<SlashCompletion>,
}

#[derive(Debug, Clone)]
pub struct SlashCompletion {
    pub prefix: String,
    pub candidates: Vec<String>,
    pub selected: usize,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let config = Config::load();
        let system_prompt = crate::agent::system_prompt::get_system_prompt();

        Self {
            config,
            mode: AgentMode::Agent,
            permission_mode: PermissionMode::Ask,
            always_allowed_tools: std::collections::HashSet::new(),
            messages: vec![ChatMessage::system(system_prompt)],
            todos: Vec::new(),
            session_id: session::new_session_id(),
            input_buffer: String::new(),
            focused_panel: FocusedPanel::Input,
            active_modal: ActiveModal::None,
            is_streaming: false,
            streaming_buffer: String::new(),
            pending_tool_calls: Vec::new(),
            tool_queue: VecDeque::new(),
            tools_in_flight: 0,
            generation: GenerationControl::new(),
            follow_chat: true,
            status_message: None,
            chat_scroll: 0,
            input_history: Vec::new(),
            history_index: None,
            should_quit: false,
            token_usage: TokenUsage::default(),
            session_token_usage: TokenUsage::default(),
            pending_turn_usage: None,
            slash_completion: None,
        }
    }

    pub fn to_session(&self) -> Session {
        Session {
            id: self.session_id.clone(),
            title: session::title_from_messages(&self.messages),
            updated_unix: session::now_unix(),
            cwd: crate::agent::workspace::workspace_root()
                .to_string_lossy()
                .to_string(),
            messages: self.messages.clone(),
            todos: self.todos.clone(),
            input_history: self.input_history.clone(),
            token_usage: self.session_token_usage.clone(),
        }
    }

    pub fn persist_session(&self) {
        if self
            .messages
            .iter()
            .all(|m| m.role != crate::provider::Role::User)
        {
            return;
        }
        let _ = session::save(&self.to_session());
    }

    pub fn apply_session(&mut self, session: Session) {
        self.generation.cancel();
        self.session_id = session.id;
        self.messages = session.messages;
        if self.messages.is_empty() || self.messages[0].role != crate::provider::Role::System {
            let system_prompt = crate::agent::system_prompt::get_system_prompt();
            self.messages.insert(0, ChatMessage::system(system_prompt));
        }
        self.todos = session.todos;
        self.input_history = session.input_history;
        self.streaming_buffer.clear();
        self.pending_tool_calls.clear();
        self.tool_queue.clear();
        self.tools_in_flight = 0;
        self.is_streaming = false;
        self.follow_chat = true;
        self.active_modal = ActiveModal::None;
        self.session_token_usage = session.token_usage;
        self.pending_turn_usage = None;
        self.slash_completion = None;
        self.status_message = Some(format!("Resumed session {}", self.session_id));
    }

    pub fn clear_conversation(&mut self) {
        self.persist_session();
        self.generation.cancel();
        let system_prompt = crate::agent::system_prompt::get_system_prompt();
        self.messages = vec![ChatMessage::system(system_prompt)];
        self.todos.clear();
        self.session_id = session::new_session_id();
        self.streaming_buffer.clear();
        self.pending_tool_calls.clear();
        self.tool_queue.clear();
        self.tools_in_flight = 0;
        self.is_streaming = false;
        self.status_message = None;
        self.follow_chat = true;
        self.active_modal = ActiveModal::None;
        self.session_token_usage = TokenUsage::default();
        self.pending_turn_usage = None;
        self.slash_completion = None;
    }

    pub fn cancel_generation(&mut self) {
        self.generation.cancel();
        self.is_streaming = false;
        self.pending_tool_calls.clear();
        self.tool_queue.clear();
        self.tools_in_flight = 0;
        self.pending_turn_usage = None;
        if !self.streaming_buffer.is_empty() {
            let text = std::mem::take(&mut self.streaming_buffer);
            self.messages.push(ChatMessage::assistant(format!(
                "{text}\n\n[generation cancelled]"
            )));
        }
        if matches!(self.active_modal, ActiveModal::ToolApproval { .. }) {
            self.active_modal = ActiveModal::None;
        }
        self.status_message = Some("Generation cancelled.".to_string());
        self.persist_session();
    }

    pub fn prune_messages_if_needed(&mut self) {
        if crate::agent::compact::should_compact(&self.messages) {
            self.messages = crate::agent::compact::compact_messages(&self.messages, 4, 2_000);
        }
    }
}
