use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    pub servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub fn config_file_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("polynia").join("mcp.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_file_path()
            && path.exists()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(cfg) = serde_json::from_str::<McpConfig>(&content)
        {
            return cfg;
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(path) = Self::config_file_path() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            std::fs::write(path, content)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InitializeParams {
    protocol_version: String,
    capabilities: Value,
    client_info: ClientInfo,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
struct InitializeResult {
    protocol_version: String,
    capabilities: Value,
    server_info: ServerInfo,
}

#[derive(Debug, Deserialize)]
#[expect(dead_code)]
struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct ListToolsResult {
    tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Deserialize)]
struct CallToolResult {
    content: Vec<McpToolContent>,
    is_error: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct McpToolContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    data: Option<Value>,
}

struct McpClient {
    child: Option<Child>,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<BufReader<tokio::process::ChildStdout>>,
    request_id: u64,
    initialized: bool,
}

impl McpClient {
    fn new() -> Self {
        Self {
            child: None,
            stdin: None,
            stdout: None,
            request_id: 0,
            initialized: false,
        }
    }

    async fn start(&mut self, config: &McpServerConfig) -> Result<()> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().context("Failed to spawn MCP server")?;

        let stdin = child.stdin.take().context("Failed to get stdin")?;
        let stdout = child.stdout.take().context("Failed to get stdout")?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.child = Some(child);

        self.initialize().await?;
        Ok(())
    }

    async fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = Value::Number(self.request_id.into());
        self.request_id += 1;

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params,
        };

        let request_str = serde_json::to_string(&request)?;
        let stdin = self.stdin.as_mut().context("Not connected")?;
        stdin
            .write_all(request_str.as_bytes())
            .await
            .context("Failed to write request")?;
        stdin
            .write_all(b"\n")
            .await
            .context("Failed to write newline")?;
        stdin.flush().await.context("Failed to flush stdin")?;

        let stdout = self.stdout.as_mut().context("Not connected")?;
        let mut line = String::new();
        stdout
            .read_line(&mut line)
            .await
            .context("Failed to read response")?;
        let response_line = line.trim();

        let response: JsonRpcResponse = serde_json::from_str(response_line)?;

        if let Some(error) = response.error {
            anyhow::bail!("MCP error: {} (code: {})", error.message, error.code);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response"))
    }

    async fn initialize(&mut self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: json!({}),
            client_info: ClientInfo {
                name: "orbis".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let result = self.send_request("initialize", Some(json!(params))).await?;
        let _init_result: InitializeResult = serde_json::from_value(result)?;

        self.send_request("initialized", Some(json!({}))).await?;
        self.initialized = true;
        Ok(())
    }

    async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let result = self.send_request("tools/list", None).await?;
        let tools_result: ListToolsResult = serde_json::from_value(result)?;
        Ok(tools_result.tools)
    }

    async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<String> {
        let result = self
            .send_request(
                "tools/call",
                Some(json!({"name": name, "arguments": arguments})),
            )
            .await?;
        let call_result: CallToolResult = serde_json::from_value(result)?;

        let mut output = String::new();
        for content in call_result.content {
            if content.content_type == "text" {
                if let Some(text) = content.text {
                    output.push_str(&text);
                }
            } else if content.content_type == "json"
                && let Some(data) = content.data
            {
                output.push_str(&serde_json::to_string_pretty(&data)?);
            }
        }

        if call_result.is_error.unwrap_or(false) {
            anyhow::bail!("Tool call failed: {}", output);
        }

        Ok(output)
    }

    async fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        self.initialized = false;
    }
}

static MCP_CLIENTS: LazyLock<Mutex<HashMap<String, McpClient>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn mcp_list_servers() -> Result<String> {
    let config = McpConfig::load();
    if config.servers.is_empty() {
        return Ok(
            "No MCP servers configured. Add servers to ~/.config/polynia/mcp.json".to_string(),
        );
    }

    let mut output = vec!["Configured MCP servers:".to_string()];
    for (name, server) in &config.servers {
        output.push(format!(
            "  {}: {} {}",
            name,
            server.command,
            server.args.join(" ")
        ));
    }
    Ok(output.join("\n"))
}

pub async fn mcp_connect_server(server_name: &str) -> Result<Vec<McpTool>> {
    let config = McpConfig::load();
    let server_config = config
        .servers
        .get(server_name)
        .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found", server_name))?;

    let mut clients = MCP_CLIENTS.lock().await;
    if !clients.contains_key(server_name) {
        let mut client = McpClient::new();
        client.start(server_config).await?;
        clients.insert(server_name.to_string(), client);
    }

    let client = clients.get_mut(server_name).context("Client not found")?;
    client.list_tools().await
}

pub async fn mcp_call_tool(server_name: &str, tool_name: &str, arguments: Value) -> Result<String> {
    let mut clients = MCP_CLIENTS.lock().await;
    let client = clients.get_mut(server_name).ok_or_else(|| {
        anyhow::anyhow!(
            "MCP server '{}' not connected. Run mcp_connect_server first.",
            server_name
        )
    })?;

    client.call_tool(tool_name, arguments).await
}

pub async fn mcp_disconnect_server(server_name: &str) -> Result<()> {
    let mut clients = MCP_CLIENTS.lock().await;
    if let Some(mut client) = clients.remove(server_name) {
        client.shutdown().await;
    }
    Ok(())
}

pub async fn mcp_disconnect_all() {
    let mut clients = MCP_CLIENTS.lock().await;
    for (_, mut client) in clients.drain() {
        client.shutdown().await;
    }
}

pub fn get_mcp_tools_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "mcp_list_servers".to_string(),
            description: "List all configured MCP servers.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolSpec {
            name: "mcp_connect_server".to_string(),
            description: "Connect to an MCP server and list its available tools.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string", "description": "Name of the MCP server to connect to" }
                },
                "required": ["server_name"]
            }),
        },
        ToolSpec {
            name: "mcp_call_tool".to_string(),
            description: "Call a tool on a connected MCP server.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string", "description": "Name of the MCP server" },
                    "tool_name": { "type": "string", "description": "Name of the tool to call" },
                    "arguments": { "type": "object", "description": "Arguments to pass to the tool" }
                },
                "required": ["server_name", "tool_name", "arguments"]
            }),
        },
        ToolSpec {
            name: "mcp_disconnect_server".to_string(),
            description: "Disconnect from an MCP server.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "server_name": { "type": "string", "description": "Name of the MCP server to disconnect from" }
                },
                "required": ["server_name"]
            }),
        },
    ]
}
