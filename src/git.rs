use std::{fs, path::Path, process::Command as StdCommand};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use url::Url;

use crate::{
    cli::AcquireMode,
    config::{GitConfig, ProjectConfig},
    context::RunContext,
    error::DeployError,
    process::{CommandSpec, path_to_string, require_command, run_streamed},
    ui,
};

/// Git HTTP 认证头与脱敏显示信息。
#[derive(Debug, Clone)]
struct GitAuth {
    actual_header: String,
    redacted_header: String,
}

/// 按模式获取代码，并把仓库切换到目标分支。
pub async fn acquire_source(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    require_command("git")?;

    match ctx.effective_mode() {
        AcquireMode::Clone => clone_repo(ctx, config).await?,
        AcquireMode::Pull => pull_repo(ctx, config).await?,
        AcquireMode::Auto => unreachable!("effective_mode 已经消除了 auto"),
    }

    checkout_branch(ctx, config).await?;
    Ok(())
}

async fn clone_repo(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    let git_url = ctx
        .cli
        .git_url
        .as_deref()
        .ok_or(DeployError::MissingGitUrl)?;
    let auth = build_git_auth(config.git()?, git_url)?;

    let workspace_exists = ctx.workspace_dir.exists();
    let workspace_has_content =
        workspace_exists && fs::read_dir(&ctx.workspace_dir)?.next().is_some();

    if workspace_has_content {
        if !ctx.cli.force_clean {
            return Err(DeployError::WorkspaceNotEmpty {
                path: ctx.workspace_dir.clone(),
            }
            .into());
        }

        ui::print_info(
            "AcquireSource",
            &format!("清理部署工作目录：{}", ctx.workspace_dir.display()),
        );
        fs::remove_dir_all(&ctx.workspace_dir)?;
    }

    run_streamed(&CommandSpec {
        stage: "AcquireSource",
        program: "git".to_string(),
        args: vec![
            "-c".to_string(),
            auth.actual_header.clone(),
            "clone".to_string(),
            git_url.to_string(),
            path_to_string(&ctx.workspace_dir),
        ],
        display_override: Some(format!(
            "git -c {} clone {} {}",
            auth.redacted_header,
            git_url,
            path_to_string(&ctx.workspace_dir)
        )),
        workdir: Some(ctx.base_dir.clone()),
        envs: vec![],
        stdin_text: None,
    })
    .await
}

async fn pull_repo(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    if !ctx.workspace_dir.join(".git").is_dir() {
        return Err(DeployError::ProjectMismatch {
            message: format!(
                "工作目录 `{}` 不是 Git 仓库，无法执行 pull",
                ctx.workspace_dir.display()
            ),
        }
        .into());
    }

    fetch_remote(ctx, config).await
}

async fn checkout_branch(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    fetch_remote(ctx, config).await?;

    let branch = &ctx.cli.branch;
    if local_branch_exists(ctx.repo_dir(), branch)? {
        run_streamed(&CommandSpec {
            stage: "AcquireSource",
            program: "git".to_string(),
            args: vec!["checkout".to_string(), branch.clone()],
            display_override: None,
            workdir: Some(ctx.workspace_dir.clone()),
            envs: vec![],
            stdin_text: None,
        })
        .await?;
    } else if remote_branch_exists(ctx.repo_dir(), branch)? {
        run_streamed(&CommandSpec {
            stage: "AcquireSource",
            program: "git".to_string(),
            args: vec![
                "checkout".to_string(),
                "-B".to_string(),
                branch.clone(),
                format!("origin/{branch}"),
            ],
            display_override: None,
            workdir: Some(ctx.workspace_dir.clone()),
            envs: vec![],
            stdin_text: None,
        })
        .await?;
    } else {
        return Err(DeployError::BranchNotFound {
            branch: branch.clone(),
        }
        .into());
    }

    if matches!(ctx.effective_mode(), AcquireMode::Pull) {
        let auth = auth_for_origin(config, ctx.repo_dir())?;
        run_streamed(&CommandSpec {
            stage: "AcquireSource",
            program: "git".to_string(),
            args: vec![
                "-c".to_string(),
                auth.actual_header.clone(),
                "pull".to_string(),
                "origin".to_string(),
                branch.clone(),
            ],
            display_override: Some(format!(
                "git -c {} pull origin {}",
                auth.redacted_header, branch
            )),
            workdir: Some(ctx.workspace_dir.clone()),
            envs: vec![],
            stdin_text: None,
        })
        .await?;
    }

    Ok(())
}

async fn fetch_remote(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    let auth = auth_for_origin(config, ctx.repo_dir())?;
    run_streamed(&CommandSpec {
        stage: "AcquireSource",
        program: "git".to_string(),
        args: vec![
            "-c".to_string(),
            auth.actual_header.clone(),
            "fetch".to_string(),
            "--all".to_string(),
            "--prune".to_string(),
        ],
        display_override: Some(format!(
            "git -c {} fetch --all --prune",
            auth.redacted_header
        )),
        workdir: Some(ctx.workspace_dir.clone()),
        envs: vec![],
        stdin_text: None,
    })
    .await
}

fn auth_for_origin(config: &ProjectConfig, repo_dir: &Path) -> Result<GitAuth> {
    let origin_url = current_remote_url(repo_dir)?;
    build_git_auth(config.git()?, &origin_url)
}

fn build_git_auth(config: &GitConfig, raw_url: &str) -> Result<GitAuth> {
    validate_git_remote(config, raw_url)?;
    let encoded = STANDARD.encode(format!("{}:{}", config.username, config.password));
    Ok(GitAuth {
        actual_header: format!("http.extraHeader=AUTHORIZATION: Basic {encoded}"),
        redacted_header: "http.extraHeader=AUTHORIZATION: Basic ***".to_string(),
    })
}

fn validate_git_remote(config: &GitConfig, raw_url: &str) -> Result<()> {
    let target = Url::parse(raw_url).map_err(|_| DeployError::UnsupportedGitUrl {
        url: raw_url.to_string(),
    })?;
    let base = Url::parse(&config.base_url).map_err(|_| DeployError::CredentialFormat {
        message: "git.base_url 不是合法 URL".to_string(),
    })?;

    if !is_supported_http_scheme(target.scheme())
        || !is_supported_http_scheme(base.scheme())
        || target.scheme() != base.scheme()
    {
        return Err(DeployError::UnsupportedGitUrl {
            url: raw_url.to_string(),
        }
        .into());
    }

    if target.host_str() != base.host_str()
        || target.port_or_known_default() != base.port_or_known_default()
    {
        return Err(DeployError::GitBaseUrlMismatch {
            url: raw_url.to_string(),
        }
        .into());
    }

    let base_path = base.path().trim_end_matches('/');
    if !base_path.is_empty() && base_path != "/" && !target.path().starts_with(base_path) {
        return Err(DeployError::GitBaseUrlMismatch {
            url: raw_url.to_string(),
        }
        .into());
    }

    Ok(())
}

fn is_supported_http_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https")
}

fn current_remote_url(repo_dir: &Path) -> Result<String> {
    let output = StdCommand::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo_dir)
        .output()
        .context("读取 origin 远程地址失败")?;

    if !output.status.success() {
        return Err(DeployError::MissingGitRemoteUrl.into());
    }

    let remote = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if remote.is_empty() {
        return Err(DeployError::MissingGitRemoteUrl.into());
    }

    Ok(remote)
}

fn local_branch_exists(repo_dir: &Path, branch: &str) -> Result<bool> {
    Ok(run_git_status(
        repo_dir,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )?
    .success())
}

fn remote_branch_exists(repo_dir: &Path, branch: &str) -> Result<bool> {
    Ok(run_git_status(
        repo_dir,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/remotes/origin/{branch}"),
        ],
    )?
    .success())
}

fn run_git_status(repo_dir: &Path, args: &[&str]) -> Result<std::process::ExitStatus> {
    let status = StdCommand::new("git")
        .args(args)
        .current_dir(repo_dir)
        .status()
        .with_context(|| format!("执行 git {:?} 失败", args))?;
    Ok(status)
}

/// 获取当前 HEAD 的短提交哈希，用于默认版本号生成。
pub fn current_short_sha(repo_dir: &Path) -> Result<String> {
    let output = StdCommand::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(repo_dir)
        .output()
        .context("读取 Git 提交哈希失败")?;

    if !output.status.success() {
        return Err(DeployError::CommandFailed {
            stage: "AcquireSource".to_string(),
            command: "git rev-parse --short HEAD".to_string(),
            exit_code: output.status.code(),
            stderr_tail: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        }
        .into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use crate::config::GitConfig;

    use super::{build_git_auth, validate_git_remote};

    #[test]
    fn reject_non_http_url() {
        let config = GitConfig {
            base_url: "https://git.example.com".to_string(),
            username: "ci".to_string(),
            password: "pwd".to_string(),
        };

        assert!(validate_git_remote(&config, "ssh://git.example.com/demo/repo.git").is_err());
    }

    #[test]
    fn build_auth_header_without_leaking_password() {
        let config = GitConfig {
            base_url: "https://git.example.com/scm".to_string(),
            username: "ci".to_string(),
            password: "pwd".to_string(),
        };

        let auth =
            build_git_auth(&config, "https://git.example.com/scm/demo/repo.git").expect("auth");
        assert!(auth.actual_header.contains("Basic "));
        assert_eq!(
            auth.redacted_header,
            "http.extraHeader=AUTHORIZATION: Basic ***"
        );
    }

    #[test]
    fn accept_http_url() {
        let config = GitConfig {
            base_url: "http://git.example.com/scm".to_string(),
            username: "ci".to_string(),
            password: "pwd".to_string(),
        };

        assert!(validate_git_remote(&config, "http://git.example.com/scm/demo/repo.git").is_ok());
    }
}
