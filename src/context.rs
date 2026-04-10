use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Result, ensure};

use crate::{
    cli::{AcquireMode, Cli, JavaLayout, ProjectType},
    error::DeployError,
};

/// 原始 CLI 参数经校验后的运行上下文。
///
/// 这里统一维护所有路径和关键部署元数据，避免各模块重复拼接路径。
#[derive(Debug, Clone)]
pub struct RunContext {
    pub cli: Cli,
    pub base_dir: PathBuf,
    pub workspace_dir: PathBuf,
    pub docker_config_dir: PathBuf,
}

/// Java 构建工具的最终选择结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveBuildTool {
    Maven,
    Gradle,
}

/// 解析后的项目信息，后续构建阶段直接消费这个结构。
#[derive(Debug, Clone)]
pub struct ProjectSpec {
    pub project_type: ProjectType,
    pub java_layout: Option<JavaLayout>,
    pub module_name: Option<String>,
    pub build_tool: Option<EffectiveBuildTool>,
    pub build_tool_command: Option<PathBuf>,
    pub project_root: PathBuf,
    pub dockerfile_path: PathBuf,
    pub build_context_dir: PathBuf,
    pub artifact_search_dir: Option<PathBuf>,
}

/// 镜像元数据，供构建与推送完成后回显。
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    pub image: String,
    pub tag: String,
    pub full_name: String,
    pub branch: String,
    pub short_sha: String,
}

impl RunContext {
    /// 基于 CLI 参数构造受限运行上下文，并保证所有路径都在当前目录内。
    pub fn from_cli(cli: Cli) -> Result<Self> {
        validate_build_args(&cli.build_args)?;

        let base_dir = std::env::current_dir()?;
        let workspace_dir = normalize_path(&base_dir.join(&cli.workspace_dir));
        ensure_within(&base_dir, &workspace_dir)?;

        let docker_config_dir = workspace_dir.join(".docker");
        Ok(Self {
            cli,
            base_dir,
            workspace_dir,
            docker_config_dir,
        })
    }

    /// 根据是否已存在 .git 目录，确定最终代码获取模式。
    pub fn effective_mode(&self) -> AcquireMode {
        match self.cli.mode {
            AcquireMode::Auto => {
                if self.workspace_dir.join(".git").is_dir() {
                    AcquireMode::Pull
                } else {
                    AcquireMode::Clone
                }
            }
            mode => mode,
        }
    }

    /// 当前部署的仓库根目录始终就是实际工作目录。
    pub fn repo_dir(&self) -> &Path {
        &self.workspace_dir
    }

    /// 如有需要，创建部署工作目录。
    pub fn ensure_workspace_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.workspace_dir)?;
        Ok(())
    }
}

/// 校验 `--build-arg` 是否满足 `KEY=VALUE` 结构。
pub fn validate_build_args(build_args: &[String]) -> Result<()> {
    for item in build_args {
        if !item.contains('=') {
            return Err(DeployError::InvalidArgument {
                message: format!("--build-arg `{item}` 缺少 `=`，应为 KEY=VALUE"),
            }
            .into());
        }
    }
    Ok(())
}

/// 以纯词法方式规范化路径，避免未创建目录无法 canonicalize 的问题。
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized
}

/// 校验目标路径必须位于当前工作目录之内。
pub fn ensure_within(base: &Path, target: &Path) -> Result<()> {
    let base = normalize_path(base);
    let target = normalize_path(target);

    ensure!(
        target.starts_with(&base),
        DeployError::PathOutsideWorkspace {
            path: target.to_path_buf()
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ensure_within, normalize_path, validate_build_args};

    #[test]
    fn normalize_windows_like_path() {
        let path = PathBuf::from(r"C:\workspace\demo\.\child\..\repo");
        assert_eq!(
            normalize_path(&path),
            PathBuf::from(r"C:\workspace\demo\repo")
        );
    }

    #[test]
    fn validate_build_args_requires_equal_sign() {
        let result = validate_build_args(&["FOO".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn reject_path_outside_workspace() {
        let base = PathBuf::from(r"C:\workspace\demo");
        let target = PathBuf::from(r"C:\workspace\other");
        assert!(ensure_within(&base, &target).is_err());
    }
}
