use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use learn_business::ai::AiRuntime;
use learn_business::config::APP_NAME;
use learn_business::ingest::run_ingest;
use learn_business::qa::answer_workspace;
use learn_business::report::report_workspace;
use learn_business::store::MetadataStore;
use learn_business::task::{CommandPermissionPolicy, PermissionSet, run_with_permissions};
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
        #[arg(long)]
        trace: Option<String>,
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
    let grants = PermissionSet::trusted_cli_defaults();
    match cli.command {
        Commands::Init { workspace } => {
            run_with_permissions(&CommandPermissionPolicy::init(), &grants, || {
                let workspace = Workspace::init(workspace)?;
                println!("初始化工作区: {}", workspace.root().display());
                Ok(())
            })?;
        }
        Commands::Ingest {
            docs_dir,
            workspace,
        } => {
            run_with_permissions(&CommandPermissionPolicy::ingest(), &grants, || {
                let summary = run_ingest(&workspace, &docs_dir)?;
                println!(
                    "ingest 完成: scanned={} indexed={} skipped={} warnings={}",
                    summary.scanned, summary.indexed, summary.skipped, summary.warnings
                );
                Ok(())
            })?;
        }
        Commands::Status { workspace } => {
            run_with_permissions(&CommandPermissionPolicy::status(), &grants, || {
                println!("工作区状态: {}", workspace.display());
                Ok(())
            })?;
        }
        Commands::InspectAi { workspace, trace } => {
            run_with_permissions(&CommandPermissionPolicy::inspect_ai(), &grants, || {
                inspect_ai_command(workspace, trace)
            })?;
        }
        Commands::Report { workspace, out } => {
            run_with_permissions(&CommandPermissionPolicy::report(), &grants, || {
                report_workspace(&workspace, &out)?;
                println!("生成报告: {}", out.display());
                Ok(())
            })?;
        }
        Commands::Ask {
            workspace,
            question,
        } => {
            run_with_permissions(&CommandPermissionPolicy::ask(), &grants, || {
                let answer = answer_workspace(&workspace, &question)?;
                println!("{}", answer.answer);
                if !answer.citations.is_empty() {
                    println!("来源:");
                    for citation in answer.citations {
                        let mut parts = vec![
                            citation.document_path,
                            format!("chunk={}", citation.chunk_id),
                            format!("score={:.4}", citation.score),
                        ];
                        if let Some(page) = citation.page {
                            parts.push(format!("page={page}"));
                        }
                        if let Some(slide) = citation.slide {
                            parts.push(format!("slide={slide}"));
                        }
                        if let Some(source_range) = citation.source_range {
                            parts.push(format!("range={source_range}"));
                        }
                        println!("- {}", parts.join(" "));
                    }
                }
                Ok(())
            })?;
        }
        Commands::DescribeImage {
            image_path,
            workspace,
            dry_run_ai,
        } => {
            run_with_permissions(
                &CommandPermissionPolicy::describe_image(dry_run_ai),
                &grants,
                || describe_image_command(image_path, workspace, dry_run_ai),
            )?;
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

fn inspect_ai_command(workspace_root: PathBuf, trace: Option<String>) -> Result<()> {
    let workspace = Workspace::open(workspace_root);
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let calls = store
        .list_ai_calls()?
        .into_iter()
        .filter(|call| match trace.as_deref() {
            Some(trace_id) => call.trace_id.as_deref() == Some(trace_id),
            None => true,
        })
        .collect::<Vec<_>>();
    if calls.is_empty() {
        println!("没有 AI 调用记录。");
        return Ok(());
    }

    for call in calls {
        println!(
            "purpose={} provider={} model={} status={} trace_id={} input_hash={} output_hash={} redaction={} token_estimate={} error_category={}",
            call.purpose,
            call.provider,
            call.model,
            call.status,
            call.trace_id.as_deref().unwrap_or("-"),
            call.input_hash,
            call.output_hash.as_deref().unwrap_or("-"),
            call.redaction_applied,
            call.token_estimate.unwrap_or(0),
            call.error_category.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}
