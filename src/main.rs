use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use learn_business::ai::AiRuntime;
use learn_business::config::{APP_NAME, AppConfig};
use learn_business::ingest::{IngestOptions, run_ingest_with_options};
use learn_business::qa::answer_workspace;
use learn_business::report::report_workspace;
use learn_business::store::MetadataStore;
use learn_business::task::{CommandPermissionPolicy, PermissionSet, run_with_permissions};
use learn_business::trace::{
    OperationTraceEvent, OperationTraceLogger, hash_text, new_operation_trace_id,
};
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
        #[arg(long)]
        describe_images: bool,
        #[arg(long)]
        dry_run_ai: bool,
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
    InspectTrace {
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
    Search {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long, default_value_t = 5)]
        limit: usize,
        query: String,
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
            describe_images,
            dry_run_ai,
        } => {
            run_with_permissions(
                &CommandPermissionPolicy::ingest_with_options(describe_images, dry_run_ai),
                &grants,
                || {
                    let summary = run_ingest_with_options(
                        &workspace,
                        &docs_dir,
                        IngestOptions {
                            describe_images,
                            dry_run_ai,
                        },
                    )?;
                    println!(
                        "ingest 完成: scanned={} indexed={} skipped={} warnings={}",
                        summary.scanned, summary.indexed, summary.skipped, summary.warnings
                    );
                    Ok(())
                },
            )?;
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
        Commands::InspectTrace { workspace, trace } => {
            run_with_permissions(&CommandPermissionPolicy::inspect_trace(), &grants, || {
                inspect_trace_command(workspace, trace)
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
                if !answer.reasoning_steps.is_empty() || answer.trace_id.is_some() {
                    println!("推算过程:");
                    if let Some(trace_id) = answer.trace_id.as_deref() {
                        println!("- trace_id={trace_id}");
                    }
                    for step in &answer.reasoning_steps {
                        println!("- {} status={} {}", step.step, step.status, step.detail);
                    }
                }
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
        Commands::Search {
            workspace,
            limit,
            query,
        } => {
            run_with_permissions(&CommandPermissionPolicy::search(), &grants, || {
                search_command(workspace, &query, limit)
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

fn search_command(workspace_root: PathBuf, query: &str, limit: usize) -> Result<()> {
    let workspace = Workspace::open(workspace_root);
    let config = AppConfig::load_or_default(workspace.config_path())?;
    let query_hash = hash_text(query);
    let trace_id = new_operation_trace_id("search", &query_hash);
    let logger = OperationTraceLogger::new(
        workspace.operation_trace_log_path(),
        config.logging.trace_enabled,
    );
    let store = MetadataStore::open(workspace.metadata_db_path())?;
    let results = store.search_text(query, limit)?;
    let mut event =
        OperationTraceEvent::new(&trace_id, "search", "store", "search_text", "completed");
    event.input_hash = Some(query_hash);
    event.result_count = Some(results.len());
    event.message = Some(format!("limit={limit}"));
    logger.append(&event)?;
    if results.is_empty() {
        println!("没有检索命中。");
        return Ok(());
    }

    for result in results {
        let mut parts = vec![
            result.document_path,
            format!("chunk={}", result.chunk_id),
            format!("kind={}", result.kind),
            format!("score={:.4}", result.score),
            format!("ai_generated={}", result.ai_generated),
        ];
        if let Some(page) = result.page {
            parts.push(format!("page={page}"));
        }
        if let Some(slide) = result.slide {
            parts.push(format!("slide={slide}"));
        }
        if let Some(source_range) = result.source_range {
            parts.push(format!("range={source_range}"));
        }
        if let Some(artifact_path) = result.artifact_path {
            parts.push(format!("artifact={artifact_path}"));
        }
        parts.push(format!("snippet={}", one_line(&result.snippet)));
        println!("- {}", parts.join(" "));
    }
    Ok(())
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn inspect_trace_command(workspace_root: PathBuf, trace: Option<String>) -> Result<()> {
    let workspace = Workspace::open(workspace_root);
    let config = AppConfig::load_or_default(workspace.config_path())?;
    let logger = OperationTraceLogger::new(
        workspace.operation_trace_log_path(),
        config.logging.trace_enabled,
    );
    let events = logger.read(trace.as_deref())?;
    if events.is_empty() {
        println!("没有步骤日志记录。");
        return Ok(());
    }

    for event in events {
        println!(
            "trace_id={} operation={} component={} step={} status={} input_hash={} output_hash={} result_count={} token_estimate={} redaction={} elapsed_ms={} error_category={} message={}",
            event.trace_id,
            event.operation,
            event.component,
            event.step,
            event.status,
            event.input_hash.as_deref().unwrap_or("-"),
            event.output_hash.as_deref().unwrap_or("-"),
            event
                .result_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            event
                .token_estimate
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            event
                .redaction_applied
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            event
                .elapsed_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            event.error_category.as_deref().unwrap_or("-"),
            event.message.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}
