use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// 命令行入口参数。
///
/// 这里尽量把部署所需的意图都收敛成显式参数，避免执行过程中再做含糊判断。
#[derive(Debug, Clone, Parser)]
#[command(
    name = "deploy-sc",
    version,
    about = "精细化自动化部署 CLI，覆盖代码获取、Java 构建、Docker 构建与镜像推送"
)]
pub struct Cli {
    /// Git 仓库地址，用于定位部署项目目录。
    #[arg(long)]
    pub git_url: Option<String>,

    /// 目标分支，默认 master。
    #[arg(long, default_value = "master")]
    pub branch: String,

    /// 项目类型，显式指定时优先于自动识别。
    #[arg(long, value_enum)]
    pub project_type: Option<ProjectType>,

    /// Java 项目布局，单模块/多模块。
    #[arg(long, value_enum)]
    pub java_layout: Option<JavaLayout>,

    /// 多模块部署时的目标模块名。
    #[arg(long)]
    pub module: Option<String>,

    /// Java 构建工具，默认自动识别 wrapper 后再回退系统命令。
    #[arg(long, value_enum, default_value_t = BuildToolArg::Auto)]
    pub build_tool: BuildToolArg,

    /// Java 打包使用的 JDK 根目录，例如 /Library/Java/JavaVirtualMachines/.../Contents/Home。
    #[arg(long)]
    pub java_home: Option<PathBuf>,

    /// 镜像仓库名，例如 registry.example.com/team/app。
    #[arg(long)]
    pub image: String,

    /// 镜像 tag，可选。未提供时自动生成 分支-短提交哈希-时间戳。
    #[arg(long)]
    pub tag: Option<String>,

    /// 透传给 docker build 的 --build-arg，可多次传递。
    #[arg(long = "build-arg")]
    pub build_args: Vec<String>,

    /// 代码获取模式：全新克隆、增量拉取或自动识别。
    #[arg(long, value_enum, default_value_t = AcquireMode::Auto)]
    pub mode: AcquireMode,

    /// 非空工作目录下允许清空后再执行 clone。
    #[arg(long)]
    pub force_clean: bool,

    /// 部署工作区根目录，项目仓库会落在其子目录中。
    #[arg(long, default_value = ".deploy-workspace")]
    pub workspace_dir: PathBuf,

    /// 跳过 K8s 部署阶段，仅构建并推送镜像。
    #[arg(long)]
    pub skip_k8s: bool,

    /// K8s 部署等待 Rollout 就绪的超时秒数。
    #[arg(long, default_value = "300")]
    pub k8s_timeout: u64,
}

/// 支持的项目类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProjectType {
    Web,
    Java,
}

/// Java 项目布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum JavaLayout {
    Single,
    Multi,
}

/// Java 构建工具。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BuildToolArg {
    Auto,
    Maven,
    Gradle,
}

/// 代码获取模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AcquireMode {
    Auto,
    Clone,
    Pull,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{AcquireMode, BuildToolArg, Cli, ProjectType};

    #[test]
    fn parse_basic_arguments() {
        let cli = Cli::parse_from([
            "deploy-sc",
            "--git-url",
            "https://example.com/repo.git",
            "--image",
            "registry.example.com/demo/app",
            "--project-type",
            "web",
        ]);

        assert_eq!(cli.git_url.as_deref(), Some("https://example.com/repo.git"));
        assert_eq!(cli.image, "registry.example.com/demo/app");
        assert_eq!(cli.branch, "master");
        assert_eq!(cli.project_type, Some(ProjectType::Web));
        assert_eq!(cli.build_tool, BuildToolArg::Auto);
        assert_eq!(cli.java_home, None);
        assert_eq!(cli.mode, AcquireMode::Auto);
        assert!(!cli.skip_k8s);
        assert_eq!(cli.k8s_timeout, 300);
    }
}
