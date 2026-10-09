# Orbis

A terminal coding agent that builds itself. Like Claude Code, Codex CLI, or opencode — but open source, self-hosting and self-building.

![GitHub Health](https://shieldcn.dev/group/github/stars/buboterrae/Orbis+github/forks/buboterrae/Orbis+github/open-issues/buboterrae/Orbis.svg?variant=secondary&size=xs)
![GitHub Last Commit](https://shieldcn.dev/github/last-commit/buboterrae/Orbis.svg?variant=outline&size=xs)
![GitHub CI](https://shieldcn.dev/github/ci/buboterrae/Orbis.svg?variant=outline&size=xs)
![GitHub Contributors](https://shieldcn.dev/github/contributors/buboterrae/Orbis.svg?variant=outline&theme=emerald&size=xs)
![GitHub Open PRs](https://shieldcn.dev/github/open-prs/buboterrae/Orbis.svg?variant=outline&size=xs)
![Repo Views](https://shieldcn.dev/views/repo/buboterrae/Orbis.svg?variant=branded&size=xs)

![Cargo](https://shieldcn.dev/badge/Cargo-E64B11.svg?logo=rust&logoColor=fff&variant=branded&size=xs)
![Rust](https://shieldcn.dev/badge/Rust-E64B11.svg?logo=rust&logoColor=fff&variant=branded&size=xs)

![Cross Platform](https://shieldcn.dev/badge/cross-platform-brightgreen.svg?variant=outline&logo=ri%3AGoDeviceDesktop&size=xs)
![Beta](https://shieldcn.dev/badge/status-beta-blue.svg?variant=outline&size=xs)
![Vibe Coded](https://shieldcn.dev/badge/vibe-coded-7C3AED.svg?variant=secondary&size=xs)
![Code Quality: Meh](https://shieldcn.dev/badge/code%20quality-meh-orange.svg?size=xs)
![Works on My Machine](https://shieldcn.dev/badge/works%20on-my%20machine-brightgreen.svg?size=xs)
![Hopes & Dreams](https://shieldcn.dev/badge/Runs%20on-hopes%20%26%20dreams-FF69B4.svg?variant=secondary&size=xs)
![Tests: Eventually](https://shieldcn.dev/badge/tests-eventually-orange.svg?variant=outline&size=xs)

## Features

- **Multi-provider LLM support**: Google Gemini, OpenAI, Anthropic, OpenRouter, DeepSeek, and custom OpenAI-compatible endpoints (Ollama, etc.)
- **Full toolset**: Read/write/edit files, search code, run commands, git operations, web fetch, symbol finding, todo management, sub-agents
- **Smart context management**: Automatic conversation compaction preserving tool-call/result pairs
- **Session persistence**: Resume work across restarts with full history
- **TUI & headless modes**: Interactive ratatui interface or `--print` for scripting/CI
- **Configurable permissions**: Ask, auto-approve, or read-only modes

## Installation

### From Source (Recommended)

```bash
git clone https://github.com/BuboTerrae/Orbis
cd orbis
cargo install --path .
```

### Pre-built Binaries

```bash
# Linux x86_64
curl -fsSL https://github.com/BuboTerrae/Orbis/releases/latest/download/orbis-linux-x86_64 -o /usr/local/bin/orbis
chmod +x /usr/local/bin/orbis

# macOS (Apple Silicon)
curl -fsSL https://github.com/BuboTerrae/Orbis/releases/latest/download/orbis-macos-aarch64 -o /usr/local/bin/orbis
chmod +x /usr/local/bin/orbis
```

## Quick Start

```bash
# Set your API key (one-time)
export GEMINI_API_KEY="your-key"   # or OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.

# Run interactively
orbis

# Or run a one-off task (headless)
orbis -p "fix the failing tests in tests/"
```

## Configuration

Orbis stores config at `~/.config/orbis/config.toml` (created on first run).

```toml
active_provider = "Gemini"
active_model = "gemini-3.1-flash-lite"

# API keys (optional if using env vars)
gemini_api_key = ""
openai_api_key = ""
anthropic_api_key = ""
openrouter_api_key = ""
deepseek_api_key = ""
custom_api_key = ""

# Custom endpoints
openai_base_url = ""      # e.g. "https://api.openai.com/v1"
openrouter_base_url = ""  # e.g. "https://openrouter.ai/api/v1"
custom_base_url = ""      # e.g. "http://localhost:11434/v1" for Ollama
```

**Environment variables take precedence** over config file:

- `GEMINI_API_KEY` / `GOOGLE_API_KEY`

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `OPENROUTER_API_KEY`
- `DEEPSEEK_API_KEY`
- `POLYNIA_API_KEY` / `CUSTOM_API_KEY`

## Usage

### Interactive TUI (Default)

```bash
orbis
```

#### Keybindings

| Key | Action |
|-----|--------|
| `Alt+P` | Switch provider |
| `Alt+K` | Open settings (API keys, models) |
| `Alt+S` | Session picker (resume previous work) |
| `Alt+A` | Cycle permission mode (Ask → Auto → Read-only) |
| `Alt+M` | Toggle Agent ↔ Chat mode |
| `Alt+L` | Clear conversation (saves session) |
| `Alt+H` | Help modal |
| `Alt+Q` | Quit |
| `Tab` | Switch focus: Input ↔ Chat |
| `Esc` | Cancel generation / clear input |
| `Enter` | Send prompt (in input panel) |
| `↑/↓` | History (input) / Scroll (chat) |
| `PgUp/PgDn` | Fast scroll (chat) |

#### Slash Commands

| Command | Description |
|---------|-------------|
| `/provider [name]` | Switch provider (gemini, openai, anthropic, openrouter, deepseek, custom) |
| `/model <name>` | Set model |
| `/settings` / `/keys` | Open settings modal |
| `/mode [agent\|chat]` | Toggle mode |
| `/permission [ask\|auto\|readonly]` | Set permission mode |
| `/ping` | Test provider connection |
| `/tree` | Show workspace tree |
| `/new` | Start new session |
| `/sessions` | Open session picker |
| `/resume [id]` | Resume session by ID |
| `/compact` | Compact conversation history |
| `/endpoint <url>` | Set OpenAI-compatible base URL |
| `/clear` | Clear conversation |
| `/status` | Show current config |
| `/debug` | Show session debug info |

### Headless / Print Mode

```bash
# Stream final answer to stdout
orbis -p "refactor src/auth.rs to use async traits"

# Auto-approve all tools (dangerous!)
orbis --print --yolo "delete all TODO comments"

# Read-only mode (safe for analysis)
orbis --readonly -p "explain the architecture"
```

**Flags:**

- `-p, --print [prompt]` — Run without TUI, stream answer

- `--yolo, --auto` — Auto-approve write/shell tools
- `--readonly` — Block mutating tools
- `--provider NAME` — Override provider
- `--model NAME` — Override model
- `--endpoint URL` — OpenAI-compatible base URL (implies custom provider)

### Piping Input

```bash
cat issue.md | orbis -p "implement this feature"
echo "fix typo in README" | orbis --print
```

## Tools Reference

| Tool | Description |
|------|-------------|
| `read_file` | Read file with optional line range |
| `write_file` | Create/overwrite file |
| `edit_file` | Surgical string replacement |
| `list_dir` | List directory contents |
| `glob_files` | Find files by pattern (`**/*.rs`) |
| `search_code` | Grep across source files |
| `run_command` | Execute shell command |
| `git_status` | `git status` |
| `git_diff` | `git diff` |
| `git_log` | `git log` |
| `git_commit` | `git commit` |
| `fetch_url` | HTTP GET with markdown conversion |
| `find_symbols` | Find code symbols (functions, types, etc.) |
| `update_todos` | Manage in-session task list |
| `spawn_subagent` | Run nested read-only agent for investigation |

## Architecture

```bash
src/
├── main.rs           # Entry point, CLI parsing, TUI event loop
├── app.rs            # Application state, session management
├── args.rs           # CLI argument parsing
├── event.rs          # Key handling, LLM streaming, tool dispatch
├── provider/         # LLM provider implementations
│   ├── mod.rs        # Traits, types (ChatMessage, ToolCall, StreamChunk)
│   ├── config.rs     # Config loading/saving, API key resolution
│   ├── anthropic.rs  # Anthropic/Claude
│   ├── openai.rs     # OpenAI & compatible
│   ├── gemini.rs     # Google Gemini
│   ├── openrouter.rs # OpenRouter
│   ├── deepseek.rs   # DeepSeek
│   └── custom.rs     # Generic OpenAI-compatible
└── agent/
    ├── mod.rs        # Provider builder, exports
    ├── runner.rs     # Agent loop, tool execution, sub-agents
    ├── harness.rs    # Permission logic, generation control, truncation
    ├── workspace.rs  # Path resolution, glob matching, binary detection
    ├── session.rs    # Session persistence (JSON)
    ├── system_prompt.rs
    ├── compact.rs    # Conversation compaction
    └── tools/        # Tool implementations
        ├── file_ops.rs
        ├── search_ops.rs
        ├── bash_ops.rs
        ├── git_ops.rs
        ├── web_ops.rs
        ├── code_symbols.rs
        ├── todo_ops.rs
        └── subagent.rs
```

## Developing

```bash
# Build
cargo build

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Run locally
cargo run -- -p "your prompt"
```

### Adding a New Tool

1. Create `src/agent/tools/your_tool.rs`
2. Implement the tool function and `get_your_tool_specs()`
3. Register in `src/agent/tools/mod.rs`:

   ```rust
   pub mod your_tool;
   // in all_specs():
   specs.extend(your_tool::get_your_tool_specs());
   // in execute_tool():
   "your_tool" => your_tool::your_tool(&arguments).await,
   ```

### Adding a New Provider

1. Create `src/provider/your_provider.rs` implementing `LlmProvider`
2. Register in `src/agent/mod.rs::build_provider()`

## License

MIT OR Apache-2.0
