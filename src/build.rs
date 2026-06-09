use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    cli::JavaLayout,
    context::{EffectiveBuildTool, ProjectSpec, RunContext},
    error::DeployError,
    process::{CommandSpec, path_to_string, run_streamed},
    ui,
};

/// 针对不同项目类型执行构建前置步骤。
pub async fn package_if_needed(ctx: &RunContext, spec: &ProjectSpec) -> Result<Option<PathBuf>> {
    match spec.project_type {
        crate::cli::ProjectType::Web => Ok(None),
        crate::cli::ProjectType::Java => {
            let jar = package_java_project(ctx, spec).await?;
            Ok(Some(jar))
        }
    }
}

async fn package_java_project(ctx: &RunContext, spec: &ProjectSpec) -> Result<PathBuf> {
    let command_spec = build_java_command_spec(ctx, spec)?;
    run_streamed(&command_spec).await?;

    let artifact_dir = spec
        .artifact_search_dir
        .as_ref()
        .context("Java 项目缺少产物搜索目录")?;
    let jar = find_executable_jar(artifact_dir)?;
    ui::print_stage_success("PackageJava", &format!("JAR 产物已生成：{}", jar.display()));
    Ok(jar)
}

fn build_java_command_spec(ctx: &RunContext, spec: &ProjectSpec) -> Result<CommandSpec> {
    let build_tool = spec.build_tool.expect("java project must have build tool");
    let command = spec
        .build_tool_command
        .as_ref()
        .expect("java project must have tool command");
    let java_envs = ctx.java_envs();

    match (build_tool, spec.java_layout) {
        (EffectiveBuildTool::Maven, Some(JavaLayout::Single)) => {
            Ok(CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args: vec![
                    "clean".to_string(),
                    "package".to_string(),
                    "-DskipTests".to_string(),
                ],
                display_override: None,
                workdir: Some(spec.project_root.clone()),
                envs: java_envs.clone(),
                stdin_text: None,
            })
        }
        (EffectiveBuildTool::Gradle, Some(JavaLayout::Single)) => {
            Ok(CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args: vec!["build".to_string(), "-x".to_string(), "test".to_string()],
                display_override: None,
                workdir: Some(spec.project_root.clone()),
                envs: java_envs.clone(),
                stdin_text: None,
            })
        }
        (EffectiveBuildTool::Maven, Some(JavaLayout::Multi)) => {
            let module_name = spec.module_name.as_deref().unwrap_or_default();
            let module_pom = spec.build_context_dir.join("pom.xml");
            let root_pom = spec.project_root.join("pom.xml");
            let (workdir, args) = if root_pom.is_file() {
                (
                    spec.project_root.clone(),
                    vec![
                        "clean".to_string(),
                        "package".to_string(),
                        "-pl".to_string(),
                        module_name.to_string(),
                        "-am".to_string(),
                        "-DskipTests".to_string(),
                    ],
                )
            } else if module_pom.is_file() {
                (
                    spec.build_context_dir.clone(),
                    vec![
                        "-f".to_string(),
                        "pom.xml".to_string(),
                        "clean".to_string(),
                        "package".to_string(),
                        "-DskipTests".to_string(),
                    ],
                )
            } else {
                return Err(DeployError::ProjectMismatch {
                    message: "多模块 Maven 项目缺少根 pom.xml 或模块 pom.xml".to_string(),
                }
                .into());
            };

            Ok(CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args,
                display_override: None,
                workdir: Some(workdir),
                envs: java_envs.clone(),
                stdin_text: None,
            })
        }
        (EffectiveBuildTool::Gradle, Some(JavaLayout::Multi)) => {
            let root_gradle = spec.project_root.join("build.gradle");
            let root_gradle_kts = spec.project_root.join("build.gradle.kts");
            let module_name = spec.module_name.as_deref().unwrap_or_default();

            let (workdir, args) = if root_gradle.is_file() || root_gradle_kts.is_file() {
                (
                    spec.project_root.clone(),
                    vec![
                        format!(":{module_name}:build"),
                        "-x".to_string(),
                        "test".to_string(),
                    ],
                )
            } else {
                (
                    spec.build_context_dir.clone(),
                    vec!["build".to_string(), "-x".to_string(), "test".to_string()],
                )
            };

            Ok(CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args,
                display_override: None,
                workdir: Some(workdir),
                envs: java_envs,
                stdin_text: None,
            })
        }
        _ => Err(DeployError::ProjectMismatch {
            message: "Java 项目缺少可执行的构建布局信息".to_string(),
        }
        .into()),
    }
}

#[cfg(test)]
fn build_command_spec_for_test(ctx: &RunContext, spec: &ProjectSpec) -> Result<CommandSpec> {
    build_java_command_spec(ctx, spec)
}

/// 在 target/ 或 build/libs/ 中选出最合理的可执行 JAR。
pub fn find_executable_jar(artifact_dir: &Path) -> Result<PathBuf> {
    let mut candidates: Vec<(u64, PathBuf)> = vec![];

    if !artifact_dir.is_dir() {
        return Err(DeployError::JarNotFound {
            path: artifact_dir.to_path_buf(),
        }
        .into());
    }

    for entry in fs::read_dir(artifact_dir)
        .with_context(|| format!("扫描 JAR 目录失败：{}", artifact_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("jar") {
            continue;
        }
        let lower = path
            .file_name()
            .map(|name| name.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if ["sources", "javadoc", "tests", "original"]
            .iter()
            .any(|marker| lower.contains(marker))
        {
            continue;
        }
        let metadata = fs::metadata(&path)?;
        candidates.push((metadata.len(), path));
    }

    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates
        .into_iter()
        .next()
        .map(|(_, path)| path)
        .ok_or_else(|| {
            DeployError::JarNotFound {
                path: artifact_dir.to_path_buf(),
            }
            .into()
        })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::{
        cli::{AcquireMode, BuildToolArg, Cli, JavaLayout, ProjectType},
        context::{EffectiveBuildTool, ProjectSpec, RunContext},
    };

    use super::{build_command_spec_for_test, find_executable_jar};

    #[test]
    fn choose_non_sources_jar() {
        let temp = TempDir::new().expect("tempdir");
        let dir = temp.path();
        fs::write(dir.join("demo-sources.jar"), b"small").expect("write");
        fs::write(dir.join("demo.jar"), b"larger-jar").expect("write");

        let jar = find_executable_jar(dir).expect("jar");
        assert_eq!(jar.file_name().and_then(|v| v.to_str()), Some("demo.jar"));
    }

    #[test]
    fn java_home_is_injected_into_maven_command_env() {
        let temp = TempDir::new().expect("tempdir");
        std::env::set_current_dir(temp.path()).expect("cwd");

        let java_home = temp.path().join("jdk8");
        let repo_dir = temp.path().join(".deploy-workspace").join("git_example_com_team_app");
        fs::create_dir_all(java_home.join("bin")).expect("create java home");
        fs::create_dir_all(&repo_dir).expect("create repo");

        let ctx = RunContext::from_cli(Cli {
            git_url: Some("https://git.example.com/team/app.git".to_string()),
            branch: "master".to_string(),
            project_type: Some(ProjectType::Java),
            java_layout: Some(JavaLayout::Single),
            module: None,
            build_tool: BuildToolArg::Maven,
            java_home: Some(java_home.clone()),
            image: "registry.example.com/demo/app".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
        })
        .expect("context");

        let spec = ProjectSpec {
            project_type: ProjectType::Java,
            java_layout: Some(JavaLayout::Single),
            module_name: None,
            build_tool: Some(EffectiveBuildTool::Maven),
            build_tool_command: Some(java_home.join("bin").join("mvn")),
            project_root: repo_dir.clone(),
            dockerfile_path: repo_dir.join("Dockerfile"),
            build_context_dir: repo_dir.clone(),
            artifact_search_dir: Some(repo_dir.join("target")),
        };

        let command = build_command_spec_for_test(&ctx, &spec).expect("command");
        let java_bin = java_home.join("bin").to_string_lossy().to_string();
        let java_home_str = java_home.to_string_lossy().to_string();
        assert!(command
            .envs
            .iter()
            .any(|(key, value)| key == "JAVA_HOME" && value == &java_home_str));
        assert!(command
            .envs
            .iter()
            .any(|(key, value)| key == "PATH" && value.starts_with(&java_bin)));
    }
}
