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
            let module_name = resolve_java_module(ctx, &repo_dir)?;
            let module_dir = repo_dir.join(&module_name);
            if !module_dir.is_dir() {
                return Err(DeployError::ModuleNotFound {
                    module: module_name.clone(),
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

    let root_has_dockerfile = repo_dir.join("Dockerfile").is_file();
    let module_candidates = find_java_module_candidates(repo_dir)?;
    if !root_has_dockerfile && !module_candidates.is_empty() {
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

fn resolve_java_module(ctx: &RunContext, repo_dir: &Path) -> Result<String> {
    if let Some(module) = &ctx.cli.module {
        return Ok(module.clone());
    }

    let candidates = find_java_module_candidates(repo_dir)?;
    if candidates.is_empty() {
        return Err(DeployError::MissingModuleName.into());
    }

    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }

    if let Some(image_hint) = image_name_hint(&ctx.cli.image) {
        let matched: Vec<&String> = candidates
            .iter()
            .filter(|candidate| candidate.eq_ignore_ascii_case(image_hint))
            .collect();
        if matched.len() == 1 {
            return Ok(matched[0].clone());
        }
    }

    Err(DeployError::ProjectMismatch {
        message: format!(
            "检测到多个可部署 Java 模块：{}，请通过 --module 指定目标模块",
            candidates.join(", ")
        ),
    }
    .into())
}

fn find_java_module_candidates(repo_dir: &Path) -> Result<Vec<String>> {
    let mut candidates = vec![];

    for entry in
        fs::read_dir(repo_dir).with_context(|| format!("读取目录失败：{}", repo_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("Dockerfile").is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        candidates.push(name.to_string());
    }

    candidates.sort();
    Ok(candidates)
}

fn image_name_hint(image: &str) -> Option<&str> {
    image
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
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
        cli::{AcquireMode, BuildToolArg, Cli, JavaLayout, ProjectType},
        context::RunContext,
        detect::detect_project,
    };

    #[test]
    fn detect_web_project_from_explicit_type() {
        let temp = TempDir::new().expect("tempdir");
        std::env::set_current_dir(temp.path()).expect("set cwd");

        let ctx = RunContext::from_cli(Cli {
            git_url: Some("https://git.example.com/team/web.git".to_string()),
            branch: "master".to_string(),
            project_type: Some(ProjectType::Web),
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            java_home: None,
            image: "repo/app".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
            skip_k8s: false,
            k8s_timeout: 300,
        })
        .expect("context");
        fs::create_dir_all(ctx.repo_dir()).expect("create repo dir");
        fs::write(ctx.repo_dir().join("Dockerfile"), "FROM scratch").expect("write dockerfile");

        let spec = detect_project(&ctx).expect("detect project");
        assert_eq!(spec.project_type, ProjectType::Web);
    }

    #[test]
    fn detect_java_multi_module_from_unique_child_dockerfile() {
        let temp = TempDir::new().expect("tempdir");
        std::env::set_current_dir(temp.path()).expect("set cwd");

        let ctx = RunContext::from_cli(Cli {
            git_url: Some("https://git.example.com/team/java.git".to_string()),
            branch: "master".to_string(),
            project_type: Some(ProjectType::Java),
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            java_home: None,
            image: "repo/sellretail-enterprise-admin".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
            skip_k8s: false,
            k8s_timeout: 300,
        })
        .expect("context");

        let repo_dir = ctx.repo_dir();
        fs::create_dir_all(repo_dir.join("sellretail-enterprise-admin")).expect("create module");
        fs::write(repo_dir.join("pom.xml"), "<project/>").expect("write pom");
        fs::write(repo_dir.join("mvnw"), "").expect("write wrapper");
        fs::write(
            repo_dir
                .join("sellretail-enterprise-admin")
                .join("Dockerfile"),
            "FROM eclipse-temurin:17",
        )
        .expect("write dockerfile");

        let spec = detect_project(&ctx).expect("detect project");
        assert_eq!(spec.project_type, ProjectType::Java);
        assert_eq!(spec.java_layout, Some(JavaLayout::Multi));
        assert_eq!(
            spec.module_name.as_deref(),
            Some("sellretail-enterprise-admin")
        );
        assert_eq!(
            spec.dockerfile_path,
            repo_dir
                .join("sellretail-enterprise-admin")
                .join("Dockerfile")
        );
        assert_eq!(
            spec.build_context_dir,
            repo_dir.join("sellretail-enterprise-admin")
        );
    }

    #[test]
    fn detect_java_multi_module_by_image_name_when_multiple_candidates_exist() {
        let temp = TempDir::new().expect("tempdir");
        std::env::set_current_dir(temp.path()).expect("set cwd");

        let ctx = RunContext::from_cli(Cli {
            git_url: Some("https://git.example.com/team/java.git".to_string()),
            branch: "master".to_string(),
            project_type: Some(ProjectType::Java),
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            java_home: None,
            image: "registry.example.com/team/sellretail-enterprise-admin".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
            skip_k8s: false,
            k8s_timeout: 300,
        })
        .expect("context");

        let repo_dir = ctx.repo_dir();
        for module in ["sellretail-appuser-api", "sellretail-enterprise-admin"] {
            fs::create_dir_all(repo_dir.join(module)).expect("create module");
            fs::write(repo_dir.join(module).join("Dockerfile"), "FROM scratch")
                .expect("write dockerfile");
        }
        fs::write(repo_dir.join("pom.xml"), "<project/>").expect("write pom");
        fs::write(repo_dir.join("mvnw"), "").expect("write wrapper");

        let spec = detect_project(&ctx).expect("detect project");
        assert_eq!(
            spec.module_name.as_deref(),
            Some("sellretail-enterprise-admin")
        );
    }

    #[test]
    fn reject_ambiguous_java_multi_module_without_module_hint() {
        let temp = TempDir::new().expect("tempdir");
        std::env::set_current_dir(temp.path()).expect("set cwd");

        let ctx = RunContext::from_cli(Cli {
            git_url: Some("https://git.example.com/team/java.git".to_string()),
            branch: "master".to_string(),
            project_type: Some(ProjectType::Java),
            java_layout: None,
            module: None,
            build_tool: BuildToolArg::Auto,
            java_home: None,
            image: "registry.example.com/team/java-app".to_string(),
            tag: None,
            build_args: vec![],
            mode: AcquireMode::Auto,
            force_clean: false,
            workspace_dir: ".deploy-workspace".into(),
            skip_k8s: false,
            k8s_timeout: 300,
        })
        .expect("context");

        let repo_dir = ctx.repo_dir();
        for module in ["module-a", "module-b"] {
            fs::create_dir_all(repo_dir.join(module)).expect("create module");
            fs::write(repo_dir.join(module).join("Dockerfile"), "FROM scratch")
                .expect("write dockerfile");
        }
        fs::write(repo_dir.join("pom.xml"), "<project/>").expect("write pom");
        fs::write(repo_dir.join("mvnw"), "").expect("write wrapper");

        let err = detect_project(&ctx).expect_err("should reject ambiguous module");
        let message = format!("{err:#}");
        assert!(message.contains("--module"));
        assert!(message.contains("module-a"));
        assert!(message.contains("module-b"));
    }
}
