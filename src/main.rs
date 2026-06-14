use std::path::PathBuf;

use anyhow::Result;
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
            println!("初始化工作区: {}", workspace.display());
        }
        Commands::Ingest {
            docs_dir,
            workspace,
        } => {
            println!(
                "ingest 文档目录: {} -> {}",
                docs_dir.display(),
                workspace.display()
            );
        }
        Commands::Status { workspace } => {
            println!("工作区状态: {}", workspace.display());
        }
        Commands::InspectAi { workspace } => {
            println!("AI 调用检查: {}", workspace.display());
        }
        Commands::Report { workspace, out } => {
            println!("生成报告: {} -> {}", workspace.display(), out.display());
        }
        Commands::Ask {
            workspace,
            question,
        } => {
            println!("问答: {} | {}", workspace.display(), question);
        }
        Commands::DescribeImage {
            image_path,
            workspace,
            dry_run_ai,
        } => {
            println!(
                "图片描述: {} | workspace={} | dry_run_ai={}",
                image_path.display(),
                workspace.display(),
                dry_run_ai
            );
        }
    }
    Ok(())
}
