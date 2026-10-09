use anyhow::Result;
use clap::Parser;
use orbis_eval::{EvalHarness, builtin_tasks};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "orbis-eval", about = "Orbis agent evaluation harness")]
struct Args {
    #[arg(long, help = "Path to orbis binary")]
    orbis: Option<PathBuf>,
    
    #[arg(long, help = "Workspace directory")]
    workspace: Option<PathBuf>,
    
    #[arg(long, help = "Run specific task by ID")]
    task: Option<String>,
    
    #[arg(long, help = "List available tasks")]
    list: bool,
    
    #[arg(long, help = "Output results to JSON file")]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    let orbis_binary = args.orbis.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap()
            .join("target/release/orbis")
    });
    
    let workspace = args.workspace.unwrap_or_else(|| {
        std::env::temp_dir().join("orbis-eval")
    });
    
    std::fs::create_dir_all(&workspace).unwrap();
    
    if args.list {
        println!("Available tasks:");
        for task in builtin_tasks() {
            println!("  {} - {}", task.id, task.name);
        }
        return Ok(());
    }
    
    let mut harness = EvalHarness::new(orbis_binary, workspace);
    let tasks = builtin_tasks();
    
    let tasks_to_run = if let Some(task_id) = args.task {
        tasks.into_iter().filter(|t| t.id == task_id).collect()
    } else {
        tasks
    };
    
    if tasks_to_run.is_empty() {
        eprintln!("No tasks to run");
        return Ok(());
    }
    
    println!("Running {} task(s)...", tasks_to_run.len());
    
    for task in tasks_to_run {
        println!("\n--- Running: {} ---", task.name);
        let result = harness.run_task(&task).await?;
        
        let status = if result.success { "✅ PASS" } else { "❌ FAIL" };
        println!("  {} ({}ms)", status, result.duration_ms);
        
        if let Some(err) = &result.error {
            println!("  Error: {}", err);
        }
        
        for criterion in &result.criteria_results {
            let c_status = if criterion.passed { "✓" } else { "✗" };
            println!("    {} {:?}: {}", c_status, criterion.criterion, criterion.details);
        }
    }
    
    println!("\n{}", harness.generate_report());
    
    if let Some(output) = args.output {
        harness.save_results(&output)?;
        println!("Results saved to {}", output.display());
    }
    
    Ok(())
}