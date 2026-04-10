use std::time::Duration;

use anyhow::Error;
use chrono::Local;
use indicatif::{ProgressBar, ProgressStyle};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::error::DeployError;

/// 初始化 tracing，便于后续按需打开更细粒度日志。
pub fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(false)
                .without_time()
                .with_writer(std::io::stderr),
        )
        .with(EnvFilter::from_default_env())
        .try_init();
}

/// 返回格式化后的本地时间戳。
pub fn timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// 打印阶段开始日志。
pub fn print_stage_start(stage: &str, message: &str) {
    println!("[{}] {} {}", timestamp(), stage, message);
}

/// 打印阶段成功日志。
pub fn print_stage_success(stage: &str, message: &str) {
    println!("[{}] ✅ {} {}", timestamp(), stage, message);
}

/// 打印附加信息。
pub fn print_info(stage: &str, message: &str) {
    println!("[{}] {} {}", timestamp(), stage, message);
}

/// 为耗时阶段创建 spinner，让用户感知任务仍在推进。
pub fn start_spinner(stage: &str, message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(120));
    spinner.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] {msg}")
            .expect("spinner template should be valid"),
    );
    spinner.set_message(format!("{stage} {message}"));
    spinner
}

/// 输出完整错误报告，包含错误链和建议修复方案。
pub fn print_error_report(error: &Error) {
    eprintln!("[{}] ❌ 部署失败：{}", timestamp(), error);

    for (index, cause) in error.chain().skip(1).enumerate() {
        eprintln!("  {}. {}", index + 1, cause);
    }

    if let Some(deploy_error) = error.downcast_ref::<DeployError>() {
        eprintln!("建议：{}", deploy_error.suggestion());
    }
}
