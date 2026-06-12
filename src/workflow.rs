use anyhow::{Result, bail};
use chrono::Local;

use crate::{
    build,
    cli::Cli,
    config::ProjectConfig,
    context::{ImageMetadata, RunContext},
    detect, docker, git, ui,
};

/// 严格串行的部署主流程。
pub async fn run(cli: Cli) -> Result<()> {
    let ctx = RunContext::from_cli(cli)?;
    ui::print_stage_start("Preflight", "开始校验部署参数与工作目录");
    ctx.ensure_workspace_dir()?;
    let project_config = ProjectConfig::load(&ctx.base_dir)?;
    let _ = project_config.git()?;
    let _ = project_config.registry()?;
    ui::print_stage_success("Preflight", "工作目录校验完成");

    ui::print_stage_start("AcquireSource", "开始获取代码并切换目标分支");
    git::acquire_source(&ctx, &project_config).await?;
    ui::print_stage_success("AcquireSource", "代码获取完成");

    ui::print_stage_start("ValidateProject", "开始识别项目类型并校验构建上下文");
    let project_spec = detect::detect_project(&ctx)?;
    ui::print_stage_success("ValidateProject", "项目校验完成");

    ui::print_stage_start("LoadCredentials", "从当前目录配置读取 Git 与 Docker 凭证");
    ui::print_stage_success("LoadCredentials", "部署凭证读取完成");

    let short_sha = git::current_short_sha(ctx.repo_dir())?;
    let image = build_image_metadata(&ctx, &short_sha)?;

    if matches!(project_spec.project_type, crate::cli::ProjectType::Java) {
        ui::print_stage_start("PackageJava", "开始执行 Java 打包");
        let _jar = build::package_if_needed(&ctx, &project_spec).await?;
    }

    ui::print_stage_start("DockerLogin", "开始登录 Docker 仓库");
    docker::login(&ctx, &project_config).await?;
    ui::print_stage_success("DockerLogin", "Docker 仓库登录成功");

    ui::print_stage_start("BuildImage", "开始构建 Docker 镜像");
    docker::build_image(&ctx, &project_spec, &image).await?;
    ui::print_stage_success("BuildImage", "镜像构建完成");

    ui::print_stage_start("PushImage", "开始推送镜像");
    docker::push_image(&ctx, &image).await?;
    ui::print_stage_success("PushImage", "镜像推送完成");

    ui::print_stage_start("RemoveImage", "开始清理本地镜像");
    docker::remove_image(&ctx, &image).await?;
    ui::print_stage_success("RemoveImage", "本地镜像已删除");

    ui::print_stage_success(
        "ReportResult",
        &format!(
            "部署完成\n版本号={}\n镜像={}，分支={}，提交={}",
            image.tag, image.full_name, image.branch, image.short_sha
        ),
    );
    Ok(())
}

fn build_image_metadata(ctx: &RunContext, short_sha: &str) -> Result<ImageMetadata> {
    let tag = match &ctx.cli.tag {
        Some(tag) => tag.clone(),
        None => {
            let branch = sanitize_tag_part(&ctx.cli.branch);
            let timestamp = Local::now().format("%Y%m%d%H%M%S");
            format!("{branch}-{short_sha}-{timestamp}")
        }
    };

    if tag.trim().is_empty() {
        bail!("镜像 tag 不能为空");
    }

    Ok(ImageMetadata {
        image: ctx.cli.image.clone(),
        tag: tag.clone(),
        full_name: format!("{}:{}", ctx.cli.image, tag),
        branch: ctx.cli.branch.clone(),
        short_sha: short_sha.to_string(),
    })
}

fn sanitize_tag_part(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            result.push(ch);
        } else {
            result.push('-');
        }
    }
    result.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use crate::{
        cli::{AcquireMode, BuildToolArg, Cli},
        context::RunContext,
        workflow::build_image_metadata,
    };

    #[test]
    fn auto_tag_contains_sha() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        std::env::set_current_dir(temp.path()).expect("cwd");
        let ctx = RunContext::from_cli(Cli {
            git_url: Some("https://git.example.com/team/app.git".to_string()),
            branch: "feature/demo".to_string(),
            project_type: None,
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            java_home: None,
            image: "registry.example.com/demo/app".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
        })
        .expect("context");

        let image = build_image_metadata(&ctx, "abc1234").expect("metadata");
        assert!(image.tag.contains("abc1234"));
        assert!(
            image
                .full_name
                .starts_with("registry.example.com/demo/app:")
        );
    }
}
