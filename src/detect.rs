use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::{
    cli::{BuildToolArg, JavaLayout, ProjectType},
    context::{EffectiveBuildTool, ProjectSpec, RunContext, ensure_within},
    error::DeployError,
    process::require_command,
};

/// 根据 CLI 参数和目录结构识别项目类型与构建入口。
pub fn detect_project(ctx: &RunContext) -> Result<ProjectSpec> {
    let repo_dir = ctx.repo_dir().to_path_buf();
    let project_type = detect_project_type(ctx, &repo_dir)?;

    match project_type {
        ProjectType::Web => detect_web_project(ctx, repo_dir),
        ProjectType::Java => detect_java_project(ctx, repo_dir),
    }
}

fn detect_project_type(ctx: &RunContext, repo_dir: &Path) -> Result<ProjectType> {
    if let Some(project_type) = ctx.cli.project_type {
        return Ok(project_type);
    }

    if has_any_file(repo_dir, &["pom.xml", "build.gradle", "build.gradle.kts"]) {
        return Ok(ProjectType::Java);
    }

    if repo_dir.join("Dockerfile").is_file() {
        return Ok(ProjectType::Web);
    }

    Err(DeployError::ProjectMismatch {
        message: "无法自动识别项目类型，请显式传入 --project-type".to_string(),
    }
    .into())
}

fn detect_web_project(ctx: &RunContext, repo_dir: PathBuf) -> Result<ProjectSpec> {
    let dockerfile_path = repo_dir.join("Dockerfile");
    validate_dockerfile(&ctx.base_dir, &dockerfile_path)?;

    Ok(ProjectSpec {
        project_type: ProjectType::Web,
        java_layout: None,
        module_name: None,
        build_tool: None,
        build_tool_command: None,
        project_root: repo_dir.clone(),
        dockerfile_path,
        build_context_dir: repo_dir,
        artifact_search_dir: None,
    })
}

fn detect_java_project(ctx: &RunContext, repo_dir: PathBuf) -> Result<ProjectSpec> {
    let java_layout = resolve_java_layout(ctx, &repo_dir)?;
    let (build_tool, build_tool_command) = resolve_build_tool(ctx, &repo_dir, java_layout)?;

    match java_layout {
        JavaLayout::Single => {
            let build_file_ok =
                has_any_file(&repo_dir, &["pom.xml", "build.gradle", "build.gradle.kts"]);
            if !build_file_ok {
                return Err(DeployError::ProjectMismatch {
                    message: "Java 单模块项目缺少根目录 pom.xml/build.gradle".to_string(),
                }
                .into());
            }

            let dockerfile_path = repo_dir.join("Dockerfile");
            validate_dockerfile(&ctx.base_dir, &dockerfile_path)?;

            Ok(ProjectSpec {
                project_type: ProjectType::Java,
                java_layout: Some(JavaLayout::Single),
                module_name: None,
                build_tool: Some(build_tool),
                build_tool_command: Some(build_tool_command),
                project_root: repo_dir.clone(),
                dockerfile_path,
                build_context_dir: repo_dir.clone(),
                artifact_search_dir: Some(artifact_dir_for_tool(&repo_dir, build_tool)),
            })
        }
        JavaLayout::Multi => {
            let module_name = ctx
                .cli
                .module
                .clone()
                .ok_or(DeployError::MissingModuleName)?;

            let module_dir = repo_dir.join(&module_name);
            if !module_dir.is_dir() {
                return Err(DeployError::ModuleNotFound {
                    module: module_name,
                }
                .into());
            }

            let dockerfile_path = module_dir.join("Dockerfile");
            validate_dockerfile(&ctx.base_dir, &dockerfile_path)?;
            let module_has_build = has_any_file(
                &module_dir,
                &["pom.xml", "build.gradle", "build.gradle.kts"],
            );
            let root_has_build =
                has_any_file(&repo_dir, &["pom.xml", "build.gradle", "build.gradle.kts"]);
            if !module_has_build && !root_has_build {
                return Err(DeployError::ProjectMismatch {
                    message: format!("多模块目录 `{}` 缺少可识别构建文件", module_dir.display()),
                }
                .into());
            }

            Ok(ProjectSpec {
                project_type: ProjectType::Java,
                java_layout: Some(JavaLayout::Multi),
                module_name: Some(module_name),
                build_tool: Some(build_tool),
                build_tool_command: Some(build_tool_command),
                project_root: repo_dir,
                dockerfile_path,
                build_context_dir: module_dir.clone(),
                artifact_search_dir: Some(artifact_dir_for_tool(&module_dir, build_tool)),
            })
        }
    }
}

fn resolve_java_layout(ctx: &RunContext, repo_dir: &Path) -> Result<JavaLayout> {
    if let Some(layout) = ctx.cli.java_layout {
        return Ok(layout);
    }

    if ctx.cli.module.is_some() {
        return Ok(JavaLayout::Multi);
    }

    if repo_dir.join("pom.xml").is_file()
        || repo_dir.join("build.gradle").is_file()
        || repo_dir.join("build.gradle.kts").is_file()
    {
        return Ok(JavaLayout::Single);
    }

    Err(DeployError::ProjectMismatch {
        message: "无法自动判断 Java 项目是单模块还是多模块，请显式传入 --java-layout".to_string(),
    }
    .into())
}

fn resolve_build_tool(
    ctx: &RunContext,
    repo_dir: &Path,
    java_layout: JavaLayout,
) -> Result<(EffectiveBuildTool, PathBuf)> {
    let module_dir = ctx.cli.module.as_ref().map(|module| repo_dir.join(module));

    let preferred_dirs: Vec<&Path> = match (&module_dir, java_layout) {
        (Some(module_dir), JavaLayout::Multi) => vec![module_dir.as_path(), repo_dir],
        _ => vec![repo_dir],
    };

    let maven_wrapper = preferred_dirs
        .iter()
        .find_map(|dir| resolve_wrapper(dir, &["mvnw.cmd", "mvnw"]));
    let gradle_wrapper = preferred_dirs
        .iter()
        .find_map(|dir| resolve_wrapper(dir, &["gradlew.bat", "gradlew"]));

    match ctx.cli.build_tool {
        BuildToolArg::Maven => {
            if let Some(wrapper) = maven_wrapper {
                return Ok((EffectiveBuildTool::Maven, wrapper));
            }
            Ok((EffectiveBuildTool::Maven, require_command("mvn")?))
        }
        BuildToolArg::Gradle => {
            if let Some(wrapper) = gradle_wrapper {
                return Ok((EffectiveBuildTool::Gradle, wrapper));
            }
            Ok((EffectiveBuildTool::Gradle, require_command("gradle")?))
        }
        BuildToolArg::Auto => {
            if let Some(wrapper) = maven_wrapper {
                return Ok((EffectiveBuildTool::Maven, wrapper));
            }
            if let Some(wrapper) = gradle_wrapper {
                return Ok((EffectiveBuildTool::Gradle, wrapper));
            }

            if has_any_file(repo_dir, &["pom.xml"]) {
                return Ok((EffectiveBuildTool::Maven, require_command("mvn")?));
            }
            if has_any_file(repo_dir, &["build.gradle", "build.gradle.kts"]) {
                return Ok((EffectiveBuildTool::Gradle, require_command("gradle")?));
            }

            Err(DeployError::ProjectMismatch {
                message: "无法识别 Java 构建工具，请显式传入 --build-tool".to_string(),
            }
            .into())
        }
    }
}

fn resolve_wrapper(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn artifact_dir_for_tool(base_dir: &Path, tool: EffectiveBuildTool) -> PathBuf {
    match tool {
        EffectiveBuildTool::Maven => base_dir.join("target"),
        EffectiveBuildTool::Gradle => base_dir.join("build").join("libs"),
    }
}

fn validate_dockerfile(base_dir: &Path, dockerfile_path: &Path) -> Result<()> {
    ensure_within(base_dir, dockerfile_path)?;
    let metadata = fs::metadata(dockerfile_path)
        .with_context(|| format!("读取 Dockerfile 失败：{}", dockerfile_path.display()))?;
    if !metadata.is_file() {
        return Err(DeployError::ProjectMismatch {
            message: format!("Dockerfile 不存在：{}", dockerfile_path.display()),
        }
        .into());
    }
    Ok(())
}

fn has_any_file(dir: &Path, names: &[&str]) -> bool {
    names.iter().any(|name| dir.join(name).is_file())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::{
        cli::{AcquireMode, BuildToolArg, Cli, ProjectType},
        context::RunContext,
        detect::detect_project,
    };

    #[test]
    fn detect_web_project_from_explicit_type() {
        let temp = TempDir::new().expect("tempdir");
        fs::write(temp.path().join("Dockerfile"), "FROM scratch").expect("write dockerfile");
        std::env::set_current_dir(temp.path()).expect("set cwd");

        let ctx = RunContext::from_cli(Cli {
            git_url: None,
            branch: "master".to_string(),
            project_type: Some(ProjectType::Web),
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            image: "repo/app".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".".into(),
        })
        .expect("context");

        let spec = detect_project(&ctx).expect("detect project");
        assert_eq!(spec.project_type, ProjectType::Web);
    }
}
