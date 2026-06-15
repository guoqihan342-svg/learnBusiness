use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use learn_business::ai::AiRuntime;
use learn_business::config::APP_NAME;
use learn_business::ingest::run_ingest;
use learn_business::qa::answer_workspace;
use learn_business::report::report_workspace;
use learn_business::store::MetadataStore;
use learn_business::workspace::Workspace;

#[derive(Debug, Parser)]
#[command(name = APP_NAME)]
#[command(about = "本地优先、轻量、省 token 的业务理解智能体")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        workspace: PathBuf,
    },
    Ingest {
        docs_dir: PathBuf,
        #[arg(long)]
        workspace: PathBuf,
    },
    Status {
        #[arg(long)]
        workspace: PathBuf,
    },
    InspectAi {
        #[arg(long)]
        workspace: PathBuf,
    },
    Report {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Ask {
        #[arg(long)]
        workspace: PathBuf,
        question: String,
    },
    DescribeImage {
        image_path: PathBuf,
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        dry_run_ai: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { workspace } => {
            let workspace = Workspace::init(workspace)?;
            println!("初始化工作区: {}", workspace.root().display());
        }
        Commands::Ingest {
            docs_dir,
            workspace,
        } => {
            let summary = run_ingest(&workspace, &docs_dir)?;
            println!(
                "ingest 完成: scanned={} indexed={} skipped={} warnings={}",
                summary.scanned, summary.indexed, summary.skipped, summary.warnings
            );
        }
        Commands::Status { workspace } => {
            println!("工作区状态: {}", workspace.display());
        }
        Commands::InspectAi { workspace } => {
            inspect_ai_command(workspace)?;
        }
        Commands::Report { workspace, out } => {
            report_workspace(&workspace, &out)?;
            println!("生成报告: {}", out.display());
        }
        Commands::Ask {
            workspace,
            question,
        } => {
            let answer = answer_workspace(&workspace, &question)?;
            println!("{}", answer.answer);
            if !answer.sources.is_empty() {
                println!("来源:");
                for source in answer.sources {
                    println!("- {source}");
                }
            }
        }
        Commands::DescribeImage {
            image_path,
            workspace,
            dry_run_ai,
        } => {
            describe_image_command(image_path, workspace, dry_run_ai)?;
        }
    }
    Ok(())
}

fn describe_image_command(
    image_path: PathBuf,
    workspace_root: PathBuf,
    dry_run_ai: bool,
) -> Result<()> {
    let runtime = AiRuntime::open(&workspace_root)?;
    let result = runtime.describe_image(&image_path, dry_run_ai)?;
    if dry_run_ai {
        println!(
            "dry-run AI purpose={} provider={} model={} image={} input_hash={} sha256={} mime={} redaction={} token_estimate={} local_provider={}",
            result.purpose,
            result.provider,
            result.model,
            result.image_path.display(),
            result.input_hash,
            result.input_hash,
            result.mime_type,
            result.redaction_applied,
            result.token_estimate,
            result.local_provider
        );
        return Ok(());
    }

    if let Some(description) = result.description {
        println!("{description}");
    }
    Ok(())
}

fn inspect_ai_command(workspace_root: PathBuf) -> Result<()> {
    let workspace = Workspace::open(workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let calls = store.list_ai_calls()?;
    if calls.is_empty() {
        println!("没有 AI 调用记录。");
        return Ok(());
    }

    for call in calls {
        println!(
            "purpose={} provider={} model={} status={} input_hash={} output_hash={} redaction={} token_estimate={} error_category={}",
            call.purpose,
            call.provider,
            call.model,
            call.status,
            call.input_hash,
            call.output_hash.as_deref().unwrap_or("-"),
            call.redaction_applied,
            call.token_estimate.unwrap_or(0),
            call.error_category.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}
