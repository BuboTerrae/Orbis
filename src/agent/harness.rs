use crate::app::{App, PermissionMode};
use crate::provider::ToolCall;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub fn is_read_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "list_dir"
            | "glob_files"
            | "search_code"
            | "git_status"
            | "git_diff"
            | "git_log"
            | "find_symbols"
            | "fetch_url"
            | "update_todos"
    )
}

pub fn should_auto_allow(app: &App, tool_name: &str) -> bool {
    app.permission_mode == PermissionMode::AutoApprove
        || app.always_allowed_tools.contains(tool_name)
        || is_read_tool(tool_name)
}

pub fn is_blocked_in_readonly(app: &App, tool_name: &str) -> bool {
    app.permission_mode == PermissionMode::ReadOnly && !is_read_tool(tool_name)
}

#[derive(Clone)]
pub struct GenerationControl {
    pub id: Arc<AtomicU64>,
    pub cancelled: Arc<AtomicBool>,
}

impl GenerationControl {
    pub fn new() -> Self {
        Self {
            id: Arc::new(AtomicU64::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> u64 {
        self.cancelled.store(false, Ordering::SeqCst);
        self.id.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_id(&self) -> u64 {
        self.id.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.id.fetch_add(1, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn is_current(&self, generation_id: u64) -> bool {
        self.current_id() == generation_id && !self.is_cancelled()
    }
}

pub fn truncate_tool_output(output: String) -> String {
    const MAX: usize = 80_000;
    if output.len() <= MAX {
        return output;
    }
    let omitted = output.len() - MAX;
    format!(
        "{}\n\n...[truncated {} bytes of tool output]",
        &output[..MAX],
        omitted
    )
}

pub fn fallback_tool_id(index: usize) -> String {
    format!("tool_{index}_{}", chrono_like_id())
}

fn chrono_like_id() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

pub fn ensure_tool_call_id(tc: &mut ToolCall, index: usize) {
    if tc.id.trim().is_empty() {
        tc.id = fallback_tool_id(index);
    }
}
