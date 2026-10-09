use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTask {
    pub id: String,
    pub name: String,
    pub description: String,
    pub setup: Option<String>,
    pub repo: Option<String>,
    pub files: HashMap<String, String>,
    pub success_criteria: Vec<SuccessCriterion>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuccessCriterion {
    FileContains { path: String, pattern: String },
    FileNotContains { path: String, pattern: String },
    TestPasses { test_name: String },
    CommandSucceeds { command: String, args: Vec<String> },
    FileExists { path: String },
    FileEquals { path: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub task_id: String,
    pub success: bool,
    pub duration_ms: u64,
    pub tokens_used: u64,
    pub tools_called: u32,
    pub error: Option<String>,
    pub criteria_results: Vec<CriterionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub criterion: SuccessCriterion,
    pub passed: bool,
    pub details: String,
}

pub struct EvalHarness {
    orbis_binary: PathBuf,
    #[expect(dead_code)]
    workspace_dir: PathBuf,
    results: Vec<EvalResult>,
}

impl EvalHarness {
    pub fn new(orbis_binary: PathBuf, workspace_dir: PathBuf) -> Self {
        Self {
            orbis_binary,
            workspace_dir,
            results: Vec::new(),
        }
    }

    pub async fn run_task(&mut self, task: &EvalTask) -> Result<EvalResult> {
        let start = Instant::now();
        let temp_dir = TempDir::new().context("Failed to create temp dir")?;
        let task_dir = temp_dir.path().join(&task.id);

        std::fs::create_dir_all(&task_dir).context("Failed to create task dir")?;

        if let Some(repo) = &task.repo {
            let output = Command::new("git")
                .args(["clone", "--depth", "1", repo, task_dir.to_str().unwrap()])
                .output()
                .await
                .context("Failed to clone repo")?;
            if !output.status.success() {
                return Ok(EvalResult {
                    task_id: task.id.clone(),
                    success: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_used: 0,
                    tools_called: 0,
                    error: Some(format!("Git clone failed: {}", String::from_utf8_lossy(&output.stderr))),
                    criteria_results: Vec::new(),
                });
            }
        } else {
            for (path, content) in &task.files {
                let file_path = task_dir.join(path);
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent).context("Failed to create parent dirs")?;
                }
                std::fs::write(&file_path, content).context("Failed to write file")?;
            }
        }

        if let Some(setup) = &task.setup {
            let output = Command::new("sh")
                .arg("-c")
                .arg(setup)
                .current_dir(&task_dir)
                .output()
                .await
                .context("Failed to run setup")?;
            if !output.status.success() {
                return Ok(EvalResult {
                    task_id: task.id.clone(),
                    success: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_used: 0,
                    tools_called: 0,
                    error: Some(format!("Setup failed: {}", String::from_utf8_lossy(&output.stderr))),
                    criteria_results: Vec::new(),
                });
            }
        }

        let prompt = format!("{}", task.description);

        let output = Command::new(&self.orbis_binary)
            .args(["--print", &prompt])
            .current_dir(&task_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn orbis")?;

        let timeout = Duration::from_secs(task.timeout_seconds);
        let timed_output = tokio::time::timeout(timeout, output.wait_with_output()).await;

        let (_stdout, _stderr, exit_code) = match timed_output {
            Ok(Ok(output)) => (output.stdout, output.stderr, output.status.code()),
            Ok(Err(e)) => {
                return Ok(EvalResult {
                    task_id: task.id.clone(),
                    success: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_used: 0,
                    tools_called: 0,
                    error: Some(format!("Spawn error: {e}")),
                    criteria_results: Vec::new(),
                });
            }
            Err(_) => {
                return Ok(EvalResult {
                    task_id: task.id.clone(),
                    success: false,
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_used: 0,
                    tools_called: 0,
                    error: Some(format!("Timeout after {}s", task.timeout_seconds)),
                    criteria_results: Vec::new(),
                });
            }
        };

        let success = exit_code == Some(0);
        let mut criteria_results = Vec::new();

        for criterion in &task.success_criteria {
            let (passed, details) = self.check_criterion(&task_dir, criterion).await;
            criteria_results.push(CriterionResult {
                criterion: criterion.clone(),
                passed,
                details,
            });
        }

        let all_passed = success && criteria_results.iter().all(|c| c.passed);

        let result = EvalResult {
            task_id: task.id.clone(),
            success: all_passed,
            duration_ms: start.elapsed().as_millis() as u64,
            tokens_used: 0,
            tools_called: 0,
            error: if !all_passed { Some("Task failed".to_string()) } else { None },
            criteria_results,
        };

        self.results.push(result.clone());
        Ok(result)
    }

    async fn check_criterion(&self, task_dir: &Path, criterion: &SuccessCriterion) -> (bool, String) {
        match criterion {
            SuccessCriterion::FileContains { path, pattern } => {
                let file_path = task_dir.join(path);
                if !file_path.exists() {
                    return (false, format!("File {} does not exist", path));
                }
                let content = std::fs::read_to_string(&file_path).unwrap_or_default();
                let passed = content.contains(pattern);
                (passed, if passed { "Pattern found".to_string() } else { format!("Pattern '{}' not found in {}", pattern, path) })
            }
            SuccessCriterion::FileNotContains { path, pattern } => {
                let file_path = task_dir.join(path);
                if !file_path.exists() {
                    return (false, format!("File {} does not exist", path));
                }
                let content = std::fs::read_to_string(&file_path).unwrap_or_default();
                let passed = !content.contains(pattern);
                (passed, if passed { "Pattern not found".to_string() } else { format!("Pattern '{}' found in {}", pattern, path) })
            }
            SuccessCriterion::FileExists { path } => {
                let file_path = task_dir.join(path);
                (file_path.exists(), if file_path.exists() { "File exists".to_string() } else { format!("File {} does not exist", path) })
            }
            SuccessCriterion::FileEquals { path, content: expected } => {
                let file_path = task_dir.join(path);
                if !file_path.exists() {
                    return (false, format!("File {} does not exist", path));
                }
                let actual = std::fs::read_to_string(&file_path).unwrap_or_default();
                let passed = actual == *expected;
                (passed, if passed { "Content matches".to_string() } else { format!("Content mismatch for {}", path) })
            }
            SuccessCriterion::CommandSucceeds { command, args } => {
                let output = Command::new(command)
                    .args(args)
                    .current_dir(task_dir)
                    .output()
                    .await;
                match output {
                    Ok(output) => (output.status.success(), 
                        if output.status.success() { "Command succeeded".to_string() } 
                        else { format!("Command failed: {}", String::from_utf8_lossy(&output.stderr)) }),
                    Err(e) => (false, format!("Failed to run command: {e}")),
                }
            }
            SuccessCriterion::TestPasses { test_name } => {
                let output = Command::new("cargo")
                    .args(["test", test_name])
                    .current_dir(task_dir)
                    .output()
                    .await;
                match output {
                    Ok(output) => (output.status.success(),
                        if output.status.success() { "Test passed".to_string() }
                        else { format!("Test failed: {}", String::from_utf8_lossy(&output.stderr)) }),
                    Err(e) => (false, format!("Failed to run test: {e}")),
                }
            }
        }
    }

    pub fn generate_report(&self) -> String {
        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.success).count();
        let failed = total - passed;

        let mut report = String::new();
        report.push_str(&format!("# Evaluation Report\n\n"));
        report.push_str(&format!("Total: {} | Passed: {} | Failed: {}\n\n", total, passed, failed));

        for result in &self.results {
            let status = if result.success { "✅ PASS" } else { "❌ FAIL" };
            report.push_str(&format!("## {} {}\n", status, result.task_id));
            report.push_str(&format!("Duration: {}ms\n", result.duration_ms));
            if let Some(err) = &result.error {
                report.push_str(&format!("Error: {}\n", err));
            }
            for criterion in &result.criteria_results {
                let status = if criterion.passed { "✓" } else { "✗" };
                report.push_str(&format!("  {} {:?}: {}\n", status, criterion.criterion, criterion.details));
            }
            report.push_str("\n");
        }

        report
    }

    pub fn save_results(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.results)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

pub fn builtin_tasks() -> Vec<EvalTask> {
    vec![
        EvalTask {
            id: "simple_fix".to_string(),
            name: "Simple Bug Fix".to_string(),
            description: "Fix the bug in the add function that returns a + b - 1 instead of a + b".to_string(),
            setup: None,
            repo: None,
            files: {
                let mut f = HashMap::new();
                f.insert("src/lib.rs".to_string(), r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b - 1  // BUG: should be a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#.to_string());
                f.insert("Cargo.toml".to_string(), r#"
[package]
name = "eval_test"
version = "0.1.0"
edition = "2021"
"#.to_string());
                f
            },
            success_criteria: vec![
                SuccessCriterion::TestPasses { test_name: "test_add".to_string() },
                SuccessCriterion::FileContains { path: "src/lib.rs".to_string(), pattern: "a + b".to_string() },
                SuccessCriterion::FileNotContains { path: "src/lib.rs".to_string(), pattern: "- 1".to_string() },
            ],
            timeout_seconds: 60,
        },
        EvalTask {
            id: "add_feature".to_string(),
            name: "Add New Feature".to_string(),
            description: "Add a new function `multiply(a, b)` that returns a * b, with tests".to_string(),
            setup: None,
            repo: None,
            files: {
                let mut f = HashMap::new();
                f.insert("src/lib.rs".to_string(), r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#.to_string());
                f.insert("Cargo.toml".to_string(), r#"
[package]
name = "eval_test"
version = "0.1.0"
edition = "2021"
"#.to_string());
                f
            },
            success_criteria: vec![
                SuccessCriterion::FileContains { path: "src/lib.rs".to_string(), pattern: "fn multiply".to_string() },
                SuccessCriterion::FileContains { path: "src/lib.rs".to_string(), pattern: "a * b".to_string() },
                SuccessCriterion::TestPasses { test_name: "test_multiply".to_string() },
            ],
            timeout_seconds: 60,
        },
        EvalTask {
            id: "refactor_struct".to_string(),
            name: "Refactor Struct".to_string(),
            description: "Rename the field `name` to `username` in the User struct and update all usages".to_string(),
            setup: None,
            repo: None,
            files: {
                let mut f = HashMap::new();
                f.insert("src/lib.rs".to_string(), r#"
pub struct User {
    pub name: String,
    pub email: String,
}

impl User {
    pub fn new(name: String, email: String) -> Self {
        Self { name, email }
    }

    pub fn greet(&self) -> String {
        format!("Hello, {}!", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_user() {
        let u = User::new("Alice".to_string(), "alice@example.com".to_string());
        assert_eq!(u.greet(), "Hello, Alice!");
    }
}
"#.to_string());
                f.insert("Cargo.toml".to_string(), r#"
[package]
name = "eval_test"
version = "0.1.0"
edition = "2021"
"#.to_string());
                f
            },
            success_criteria: vec![
                SuccessCriterion::FileContains { path: "src/lib.rs".to_string(), pattern: "username".to_string() },
                SuccessCriterion::FileNotContains { path: "src/lib.rs".to_string(), pattern: "pub name:".to_string() },
                SuccessCriterion::TestPasses { test_name: "test_user".to_string() },
            ],
            timeout_seconds: 60,
        },
    ]
}