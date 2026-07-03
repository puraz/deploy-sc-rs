pub mod build;
pub mod cli;
pub mod config;
pub mod context;
pub mod detect;
pub mod docker;
pub mod error;
pub mod git;
pub mod k8s;
pub mod process;
pub mod ui;
pub mod workflow;

use anyhow::Result;

use crate::cli::Cli;

/// 初始化日志并执行部署主流程。
pub async fn run(cli: Cli) -> Result<()> {
    ui::init_tracing();
    workflow::run(cli).await
}
