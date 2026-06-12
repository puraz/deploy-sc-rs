use anyhow::Result;

use crate::{
    config::ProjectConfig,
    context::{ImageMetadata, ProjectSpec, RunContext, ensure_within},
    process::{CommandSpec, path_to_string, require_command, run_streamed},
};

/// 执行 docker build。
pub async fn build_image(
    ctx: &RunContext,
    spec: &ProjectSpec,
    image: &ImageMetadata,
) -> Result<()> {
    require_command("docker")?;
    ensure_within(&ctx.base_dir, &spec.dockerfile_path)?;
    ensure_within(&ctx.base_dir, &spec.build_context_dir)?;

    run_streamed(&build_image_command(ctx, spec, image)).await
}

fn build_image_command(ctx: &RunContext, spec: &ProjectSpec, image: &ImageMetadata) -> CommandSpec {
    let mut args = vec![
        "build".to_string(),
        "-f".to_string(),
        path_to_string(&spec.dockerfile_path),
        "-t".to_string(),
        image.full_name.clone(),
    ];

    for build_arg in &ctx.cli.build_args {
        args.push("--build-arg".to_string());
        args.push(build_arg.clone());
    }
    args.push(path_to_string(&spec.build_context_dir));

    CommandSpec {
        stage: "BuildImage",
        program: "docker".to_string(),
        args,
        display_override: None,
        workdir: Some(ctx.base_dir.clone()),
        envs: vec![(
            "DOCKER_CONFIG".to_string(),
            path_to_string(&ctx.docker_config_dir),
        )],
        stdin_text: None,
    }
}

/// 使用项目内凭证执行 docker login，并将配置隔离在工作目录内。
pub async fn login(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    require_command("docker")?;
    std::fs::create_dir_all(&ctx.docker_config_dir)?;
    let registry = config.registry()?;

    run_streamed(&login_command(ctx, registry)).await
}

fn login_command(ctx: &RunContext, registry: &crate::config::RegistryConfig) -> CommandSpec {
    CommandSpec {
        stage: "DockerLogin",
        program: "docker".to_string(),
        args: vec![
            "login".to_string(),
            registry.server.clone(),
            "--username".to_string(),
            registry.username.clone(),
            "--password-stdin".to_string(),
        ],
        display_override: Some(format!(
            "docker login {} --username {} --password-stdin",
            registry.server, registry.username
        )),
        workdir: Some(ctx.base_dir.clone()),
        envs: vec![(
            "DOCKER_CONFIG".to_string(),
            path_to_string(&ctx.docker_config_dir),
        )],
        stdin_text: Some(registry.registry_secret().to_string()),
    }
}

/// 推送镜像到远端仓库。
pub async fn push_image(ctx: &RunContext, image: &ImageMetadata) -> Result<()> {
    require_command("docker")?;
    run_streamed(&CommandSpec {
        stage: "PushImage",
        program: "docker".to_string(),
        args: vec!["push".to_string(), image.full_name.clone()],
        display_override: None,
        workdir: Some(ctx.base_dir.clone()),
        envs: vec![(
            "DOCKER_CONFIG".to_string(),
            path_to_string(&ctx.docker_config_dir),
        )],
        stdin_text: None,
    })
    .await
}

/// 删除本地镜像以释放磁盘空间。
pub async fn remove_image(ctx: &RunContext, image: &ImageMetadata) -> Result<()> {
    require_command("docker")?;
    run_streamed(&CommandSpec {
        stage: "RemoveImage",
        program: "docker".to_string(),
        args: vec!["rmi".to_string(), image.full_name.clone()],
        display_override: None,
        workdir: Some(ctx.base_dir.clone()),
        envs: vec![(
            "DOCKER_CONFIG".to_string(),
            path_to_string(&ctx.docker_config_dir),
        )],
        stdin_text: None,
    })
    .await
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use crate::{
        cli::{AcquireMode, BuildToolArg, Cli, ProjectType},
        config::RegistryConfig,
        context::{ImageMetadata, ProjectSpec, RunContext},
    };

    use super::{build_image_command, login_command};

    fn test_cli() -> Cli {
        Cli {
            git_url: Some("https://git.example.com/team/app.git".to_string()),
            branch: "release".to_string(),
            project_type: Some(ProjectType::Web),
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            java_home: None,
            image: "registry.example.com/team/app".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
        }
    }

    fn test_image() -> ImageMetadata {
        ImageMetadata {
            image: "registry.example.com/team/app".to_string(),
            tag: "release-abc1234".to_string(),
            full_name: "registry.example.com/team/app:release-abc1234".to_string(),
            branch: "release".to_string(),
            short_sha: "abc1234".to_string(),
        }
    }

    fn test_context(temp: &TempDir) -> RunContext {
        let base_dir = temp.path().to_path_buf();
        let workspace_dir = base_dir.join(".deploy-workspace");
        let repo_dir = workspace_dir.join("git_example_com_team_app");
        let docker_config_dir = repo_dir.join(".docker");
        RunContext {
            cli: test_cli(),
            base_dir,
            workspace_dir,
            repo_dir,
            docker_config_dir,
            java_home: None,
        }
    }

    #[test]
    fn build_image_uses_isolated_docker_config() {
        let temp = TempDir::new().expect("tempdir");
        let ctx = test_context(&temp);
        let dockerfile_path = ctx.repo_dir().join("Dockerfile");
        let spec = ProjectSpec {
            project_type: ProjectType::Web,
            java_layout: None,
            module_name: None,
            build_tool: None,
            build_tool_command: None,
            project_root: ctx.repo_dir().to_path_buf(),
            dockerfile_path,
            build_context_dir: ctx.repo_dir().to_path_buf(),
            artifact_search_dir: None,
        };

        let command = build_image_command(&ctx, &spec, &test_image());

        assert_eq!(
            command.envs,
            vec![(
                "DOCKER_CONFIG".to_string(),
                ctx.docker_config_dir.to_string_lossy().to_string(),
            )]
        );
    }

    #[test]
    fn login_uses_password_stdin_with_isolated_docker_config() {
        let temp = TempDir::new().expect("tempdir");
        let ctx = test_context(&temp);
        let registry = RegistryConfig {
            server: "registry.example.com".to_string(),
            username: "ci".to_string(),
            password: Some("secret".to_string()),
            token: None,
        };

        let command = login_command(&ctx, &registry);

        assert_eq!(command.stage, "DockerLogin");
        assert_eq!(command.stdin_text.as_deref(), Some("secret"));
        assert_eq!(
            command.display_command(),
            "docker login registry.example.com --username ci --password-stdin"
        );
        assert_eq!(
            command.envs,
            vec![(
                "DOCKER_CONFIG".to_string(),
                ctx.docker_config_dir.to_string_lossy().to_string(),
            )]
        );
    }
}
