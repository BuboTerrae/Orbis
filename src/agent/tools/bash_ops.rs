use crate::agent::workspace::workspace_root;
use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde_json::json;
use tokio::time::{Duration, timeout};

pub async fn run_command(args: &serde_json::Value) -> Result<String> {
    let command_str = args["command"]
        .as_str()
        .context("Missing 'command' argument in run_command")?;
    let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(120).clamp(1, 600);

    let child = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command_str)
        .current_dir(workspace_root())
        .kill_on_drop(true)
        .output();

    let output = match timeout(Duration::from_secs(timeout_secs), child).await {
        Ok(res) => res.with_context(|| format!("Failed to execute command: {}", command_str))?,
        Err(_) => anyhow::bail!("Command timed out after {}s: {}", timeout_secs, command_str),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status_str = if output.status.success() {
        "SUCCESS"
    } else {
        "FAILED"
    };
    let code = output
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".to_string());

    Ok(format!(
        "Command [{}] Status: {} (exit {})\n--- STDOUT ---\n{}\n--- STDERR ---\n{}",
        command_str,
        status_str,
        code,
        stdout.trim(),
        stderr.trim()
    ))
}

pub fn get_bash_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "run_command".to_string(),
        description: "Run a shell command in the workspace directory. Prefer git_* tools for git. Default timeout 120s.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command line" },
                "timeout_secs": { "type": "integer", "description": "Timeout in seconds (1-600, default 120)" }
            },
            "required": ["command"]
        }),
    }]
}
