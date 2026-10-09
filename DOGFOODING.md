# Dogfooding Orbis

Orbis is designed to build itself. This document describes how to use Orbis to develop Orbis.

## Quick Start

```bash
# Clone the repo
git clone https://github.com/your-org/orbis
cd orbis

# Build
cargo build --release

# Use Orbis to work on Orbis
./target/release/orbis
```

## Example Dogfooding Tasks

### 1. Add a New Slash Command

```bash
# Ask Orbis to add a `/git-status` slash command
./target/release/orbis -p "Add a slash command /git-status that runs git status and shows the result in the status message"
```

This will:

1. Read the event.rs file to understand slash command handling

2. Add the new command in `handle_slash_command`
3. Test it works

### 2. Fix a Bug

```bash
# Ask Orbis to fix a bug
./target/release/orbis -p "The token counter in the statusbar shows '0 tok' initially. Make it show '—' instead when no tokens have been used yet"
```

### 3. Refactor Code

```bash
# Ask Orbis to refactor
./target/release/orbis -p "Refactor the ProviderType enum to use strum for Display implementation"
```

### 4. Add Tests

```bash
# Ask Orbis to add tests
./target/release/orbis -p "Add unit tests for the git_worktree_list tool in tests/integration_tests.rs"
```

### 5. Update Documentation

```bash
# Ask Orbis to update README
./target/release/orbis -p "Update README.md to document the new /compact slash command"
```

## Development Workflow

1. **Start a session**: `orbis`
2. **Describe the task** in natural language
3. **Orbis explores** the codebase using tools (glob_files, search_code, find_symbols)
4. **Orbis implements** the changes using edit_file, write_file
5. **Orbis verifies** with cargo test, cargo clippy, cargo fmt
6. **Commit** if satisfied: `git add . && git commit -m "feat: ..."`

## Configuration for Development

Set up API keys for development:

```bash
export GEMINI_API_KEY="your-key"
# or
export OPENAI_API_KEY="your-key"
```

Or configure in `~/.config/polynia/config.toml`:

```toml
active_provider = "Gemini"
gemini_api_key = "your-key"
```

## Tips for Effective Dogfooding

1. **Be specific** in your prompts - include file paths, function names
2. **Use readonly mode** first to explore: `orbis --readonly -p "..."`
3. **Use subagents** for complex investigations: "Use spawn_subagent to investigate the token counting logic"
4. **Check the session** - Orbis persists sessions, so you can resume: `orbis --resume <id>`

## Example Session

```bash
$ orbis
> Add a slash command /tokens that shows detailed token usage breakdown

[Orbis searches for slash command handling...]
[Orbis reads event.rs...]
[Orbis edits event.rs to add /tokens command...]
[Orbis runs cargo test...]
[Orbis runs cargo clippy...]

Done! The /tokens command now shows input/output/cached token breakdown.
```

## Why This Matters

Dogfooding ensures:

- Orbis can actually do real development work

- Bugs are caught early (Orbis finds its own bugs)
- UX improvements come from real usage
- The tool evolves based on its own needs

## Current Status

Orbis can now:

- ✅ Read and write files

- ✅ Search code
- ✅ Run commands and tests
- ✅ Execute git operations
- ✅ Manage sessions
- ✅ Self-update from GitHub Releases
- ✅ Load external plugins
- ✅ Run evaluation benchmarks

The next step is using Orbis to implement the remaining features on its roadmap.
