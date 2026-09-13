use crate::provider::ToolSpec;
use serde_json::json;

pub fn get_subagent_tools_specs() -> Vec<ToolSpec> {
    vec![ToolSpec {
        name: "spawn_subagent".to_string(),
        description: "Run a nested coding agent on a focused subtask and return its final summary. Default is read-only. Do not use for the whole user request — only for isolated investigations. Nested sub-agents cannot spawn further sub-agents.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string", "description": "Complete instructions for the sub-agent" },
                "readonly": { "type": "boolean", "description": "If true (default), the sub-agent cannot write files, run mutating shell, or commit" },
                "max_turns": { "type": "integer", "description": "Tool-loop turns (default 12, max 16)" }
            },
            "required": ["prompt"]
        }),
    }]
}
