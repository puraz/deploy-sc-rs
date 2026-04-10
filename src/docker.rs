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

    run_streamed(&CommandSpec {
        stage: "BuildImage",
        program: "docker".to_string(),
        args,
        display_override: None,
        workdir: Some(ctx.base_dir.clone()),
        envs: vec![],
        stdin_text: None,
    })
    .await
}

/// 使用项目内凭证执行 docker login，并将配置隔离在工作目录内。
pub async fn login(ctx: &RunContext, config: &ProjectConfig) -> Result<()> {
    require_command("docker")?;
    std::fs::create_dir_all(&ctx.docker_config_dir)?;
    let registry = config.registry()?;

    run_streamed(&CommandSpec {
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
    })
    .await
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
