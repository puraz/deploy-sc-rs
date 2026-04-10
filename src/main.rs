use clap::Parser;
use deploy_sc::cli::Cli;
use deploy_sc::run;

/// 二进制入口只负责初始化运行时、解析参数并调用库逻辑。
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli).await {
        deploy_sc::ui::print_error_report(&error);
        std::process::exit(1);
    }
}
