use std::path::PathBuf;

use thiserror::Error;

/// 部署流程中的业务错误。
///
/// 每类错误都尽量保留足够的上下文，方便最终打印建议修复方案。
#[derive(Debug, Error)]
pub enum DeployError {
    #[error("工作目录非空，执行 clone 前请确认是否允许清空：{path}")]
    WorkspaceNotEmpty { path: PathBuf },

    #[error("首次克隆必须提供 --git-url")]
    MissingGitUrl,

    #[error("缺少 Git 凭证配置，请检查 .deploy-sc.toml 中的 [git] 节")]
    MissingGitCredential,

    #[error("目标分支不存在：{branch}")]
    BranchNotFound { branch: String },

    #[error("Git 远程 origin 未配置 URL")]
    MissingGitRemoteUrl,

    #[error("Git 仓库地址仅支持 HTTP/HTTPS：{url}")]
    UnsupportedGitUrl { url: String },

    #[error("Git 仓库地址与配置的 git.base_url 不匹配：{url}")]
    GitBaseUrlMismatch { url: String },

    #[error("缺少外部命令：{program}")]
    MissingTool { program: String },

    #[error("项目类型与目录内容不匹配：{message}")]
    ProjectMismatch { message: String },

    #[error("多模块项目缺少目标模块参数 --module")]
    MissingModuleName,

    #[error("模块不存在或不可部署：{module}")]
    ModuleNotFound { module: String },

    #[error("Docker 凭证配置文件缺失：{path}")]
    CredentialFileMissing { path: PathBuf },

    #[error("Docker 凭证配置文件格式错误：{message}")]
    CredentialFormat { message: String },

    #[error("路径超出当前工作目录限制：{path}")]
    PathOutsideWorkspace { path: PathBuf },

    #[error("未找到可执行 JAR 产物：{path}")]
    JarNotFound { path: PathBuf },

    #[error("命令执行失败：{command}")]
    CommandFailed {
        stage: String,
        command: String,
        exit_code: Option<i32>,
        stderr_tail: Option<String>,
    },

    #[error("参数不合法：{message}")]
    InvalidArgument { message: String },
}

impl DeployError {
    /// 针对不同错误类型给出明确的修复建议。
    pub fn suggestion(&self) -> String {
        match self {
            Self::WorkspaceNotEmpty { .. } => {
                "如果确认可以重建部署工作目录，请追加 --force-clean 后重试。".to_string()
            }
            Self::MissingGitUrl => {
                "首次克隆时请提供 --git-url；如果工作目录里已有仓库，可改用 --mode pull。"
                    .to_string()
            }
            Self::MissingGitCredential => {
                "请在当前执行目录的 .deploy-sc.toml 中配置 [git] 的 base_url、username、password。"
                    .to_string()
            }
            Self::BranchNotFound { .. } => {
                "请确认目标分支在远程 origin 上存在，或检查分支名是否输入错误。".to_string()
            }
            Self::MissingGitRemoteUrl => {
                "请确认仓库已配置 origin 远程地址，或在 clone 模式下显式传入 --git-url。".to_string()
            }
            Self::UnsupportedGitUrl { .. } => {
                "当前版本只支持 HTTP/HTTPS 协议的 Git 仓库地址，请改用 http://... 或 https://... 形式的仓库 URL。"
                    .to_string()
            }
            Self::GitBaseUrlMismatch { .. } => {
                "请确认 --git-url 或 origin 地址与 .deploy-sc.toml 中的 git.base_url 属于同一 Git 服务。"
                    .to_string()
            }
            Self::MissingTool { program } => format!(
                "请先在 PATH 中安装并验证 `{program}` 可执行，再重新运行部署。"
            ),
            Self::ProjectMismatch { .. } => {
                "请检查项目类型、Dockerfile、构建文件以及模块路径是否与参数一致。".to_string()
            }
            Self::MissingModuleName => {
                "多模块 Java 项目必须通过 --module 指定要部署的模块。".to_string()
            }
            Self::ModuleNotFound { .. } => {
                "请确认模块目录存在，且模块内包含 Dockerfile 与对应构建文件。".to_string()
            }
            Self::CredentialFileMissing { .. } => {
                "请在执行命令的当前目录放置 .deploy-sc.toml，并配置 [git] 与 [registry] 所需字段。"
                    .to_string()
            }
            Self::CredentialFormat { .. } => {
                "请检查 .deploy-sc.toml 字段名与值是否正确，password 与 token 二选一即可。"
                    .to_string()
            }
            Self::PathOutsideWorkspace { .. } => {
                "请将 Dockerfile、模块目录和工作目录都限制在当前项目目录内。".to_string()
            }
            Self::JarNotFound { .. } => {
                "请确认构建命令成功完成，且 Dockerfile 所需的可执行 JAR 已输出到 target/ 或 build/libs/。"
                    .to_string()
            }
            Self::CommandFailed { stage, .. } => format!(
                "请根据 `{stage}` 阶段日志检查命令输出、环境依赖和凭证配置，再重新执行。"
            ),
            Self::InvalidArgument { .. } => {
                "请修正命令行参数后重试，可先使用 --help 查看完整参数说明。".to_string()
            }
        }
    }
}
