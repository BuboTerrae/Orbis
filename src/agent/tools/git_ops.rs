use crate::agent::workspace::workspace_root;
use crate::provider::ToolSpec;
use anyhow::{Context, Result};
use git2::{Repository, Signature};
use serde_json::json;

fn get_repo() -> Result<Repository> {
    Repository::discover(workspace_root()).context("Failed to find git repository")
}

pub async fn git_status(_args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let statuses = repo.statuses(None).context("Failed to get git statuses")?;

    let mut changed_files = String::new();
    for status in statuses.iter() {
        if let Ok(path) = status.path() {
            changed_files.push_str(&format!("{:?} {:?}\n", status.status(), path));
        }
    }

    let head = repo.head().context("Failed to get HEAD")?;
    let branch = head.name().unwrap_or("unknown").replace("refs/heads/", "");

    if changed_files.is_empty() {
        Ok(format!("Branch: {}\nWorking tree clean.", branch))
    } else {
        Ok(format!(
            "Branch: {}\nChanged files:\n{}",
            branch, changed_files
        ))
    }
}

pub async fn git_diff(args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let staged = args["staged"].as_bool().unwrap_or(false);

    let mut diff_options = git2::DiffOptions::new();

    let diff = if staged {
        let head = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_index(Some(&head), None, Some(&mut diff_options))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut diff_options))?
    };

    let mut diff_output = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        diff_output.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    })?;

    if diff_output.trim().is_empty() {
        Ok("No diff output.".to_string())
    } else {
        Ok(diff_output.trim().to_string())
    }
}

pub async fn git_log(_args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let mut log_output = String::new();
    for oid in revwalk.take(5) {
        let oid = oid?;
        let commit = repo.find_commit(oid)?;
        log_output.push_str(&format!("{:.7} {:?}\n", oid, commit.summary()));
    }

    Ok(log_output.trim().to_string())
}

pub async fn git_commit(args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let message = args["message"]
        .as_str()
        .context("Missing 'message' argument in git_commit")?;

    let mut index = repo.index()?;
    index.add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let head = repo.head()?.peel_to_commit()?;
    let sig = Signature::now("Polynia", "polynia@example.com")?;

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&head])?;

    Ok("Successfully created commit.".to_string())
}

pub async fn git_worktree_list(_args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let worktree_names = repo.worktrees()?;
    let mut output = Vec::new();
    for name_result in worktree_names.iter() {
        let name = name_result?;
        if let Some(name) = name {
            let wt = repo.find_worktree(name)?;
            let path = wt.path().display().to_string();
            let branch = wt.name()?.unwrap_or("(detached)");
            output.push(format!("{}  {}", path, branch));
        }
    }
    if output.is_empty() {
        Ok("No worktrees.".to_string())
    } else {
        Ok(output.join("\n"))
    }
}

pub async fn git_worktree_add(args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in git_worktree_add")?;
    let branch = args["branch"].as_str();
    let create_branch = args["create_branch"].as_bool().unwrap_or(true);

    let path = std::path::Path::new(path_str);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("worktree");

    let mut opts = git2::WorktreeAddOptions::new();
    opts.checkout_existing(!create_branch);

    let worktree = if let Some(_branch) = branch {
        repo.worktree(name, path, Some(&opts))?
    } else {
        repo.worktree(name, path, Some(&opts))?
    };

    Ok(format!("Created worktree at {}", worktree.path().display()))
}

pub async fn git_worktree_remove(args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let path_str = args["path"]
        .as_str()
        .context("Missing 'path' argument in git_worktree_remove")?;
    let force = args["force"].as_bool().unwrap_or(false);

    let path = std::path::Path::new(path_str);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("worktree");

    let wt = repo.find_worktree(name)?;
    if force {
        wt.lock(None)?;
    }
    wt.prune(Some(
        &mut git2::WorktreePruneOptions::new().working_tree(force),
    ))?;

    Ok(format!("Removed worktree at {}", path.display()))
}

pub async fn git_worktree_prune(_args: &serde_json::Value) -> Result<String> {
    let repo = get_repo()?;
    let worktree_names = repo.worktrees()?;
    for name_result in worktree_names.iter() {
        if let Ok(Some(name)) = name_result
            && let Ok(wt) = repo.find_worktree(name)
        {
            let _ = wt.prune(Some(&mut git2::WorktreePruneOptions::new()));
        }
    }
    Ok("Pruned worktree administrative files.".to_string())
}

pub fn get_git_tools_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "git_status".to_string(),
            description: "Show working tree status and active git branch.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolSpec {
            name: "git_diff".to_string(),
            description: "Show changes in working directory or staged commits.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Optional specific file or directory path" },
                    "staged": { "type": "boolean", "description": "Set true to inspect staged changes" }
                }
            }),
        },
        ToolSpec {
            name: "git_log".to_string(),
            description: "View recent git commits.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Maximum number of commits to list (default: 5)" }
                }
            }),
        },
        ToolSpec {
            name: "git_commit".to_string(),
            description: "Stage changes and create a git commit with a message.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Commit message" },
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional specific files to stage (stages '.' if omitted)"
                    }
                },
                "required": ["message"]
            }),
        },
        ToolSpec {
            name: "git_worktree_list".to_string(),
            description: "List all git worktrees.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
        ToolSpec {
            name: "git_worktree_add".to_string(),
            description: "Create a new git worktree at the given path. Optionally specify a branch to check out or create.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path where the worktree will be created" },
                    "branch": { "type": "string", "description": "Branch to check out (creates new branch if create_branch=true)" },
                    "create_branch": { "type": "boolean", "description": "Create the branch if it doesn't exist (default: true)" }
                },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "git_worktree_remove".to_string(),
            description: "Remove a git worktree. Use force=true to remove even if there are uncommitted changes.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path of the worktree to remove" },
                    "force": { "type": "boolean", "description": "Force removal even with uncommitted changes (default: false)" }
                },
                "required": ["path"]
            }),
        },
        ToolSpec {
            name: "git_worktree_prune".to_string(),
            description: "Prune worktree administrative files (clean up removed worktrees).".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}
