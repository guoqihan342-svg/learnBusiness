use std::path::PathBuf;

use anyhow::Result;
use biz_agent::ai::cache::AiCacheKey;
use biz_agent::ai::{AiProvider, ImageInput, MockAiProvider};
use biz_agent::discover::{guess_file_type, sha256_file};
use biz_agent::ingest::run_ingest;
use biz_agent::qa::answer_workspace;
use biz_agent::report::report_workspace;
use biz_agent::store::{AiCallRecord, MetadataStore};
use biz_agent::workspace::Workspace;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "biz-agent")]
#[command(about = "本地优先的业务文档理解 agent")]
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
    let workspace = Workspace::open(&workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let content_hash = sha256_file(&image_path)?;
    let mime_type = guess_file_type(&image_path);
    if dry_run_ai {
        store.insert_ai_call(&AiCallRecord::new(
            "mock",
            "mock-ai",
            "describe_image",
            &content_hash,
            "dry_run",
        ))?;
        println!(
            "dry-run AI purpose=describe_image image={} sha256={} mime={} redaction=false token_estimate=0",
            image_path.display(),
            content_hash,
            mime_type
        );
        return Ok(());
    }

    let image = ImageInput::new(&image_path, mime_type, &content_hash);
    let provider = MockAiProvider::default();
    let understanding =
        provider.describe_image(&image, "请描述这张业务图片中的流程、角色和关键信息。")?;
    store.insert_ai_call(&AiCallRecord::new(
        "mock",
        &understanding.model,
        "describe_image",
        &content_hash,
        "completed",
    ))?;
    let cache_key = AiCacheKey::new(
        "mock",
        &understanding.model,
        "describe_image",
        "v1",
        &content_hash,
        false,
    );
    std::fs::create_dir_all(workspace.ai_cache_dir())?;
    std::fs::write(
        workspace.ai_cache_dir().join(cache_key.to_filename()),
        &understanding.description,
    )?;
    println!("{}", understanding.description);
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
            "purpose={} provider={} model={} status={} input_hash={} redaction={} token_estimate={}",
            call.purpose,
            call.provider,
            call.model,
            call.status,
            call.input_hash,
            call.redaction_applied,
            call.token_estimate.unwrap_or(0)
        );
    }
    Ok(())
}
