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
pub async fn package_if_needed(_ctx: &RunContext, spec: &ProjectSpec) -> Result<Option<PathBuf>> {
    match spec.project_type {
        crate::cli::ProjectType::Web => Ok(None),
        crate::cli::ProjectType::Java => {
            let jar = package_java_project(spec).await?;
            Ok(Some(jar))
        }
    }
}

async fn package_java_project(spec: &ProjectSpec) -> Result<PathBuf> {
    let build_tool = spec.build_tool.expect("java project must have build tool");
    let command = spec
        .build_tool_command
        .as_ref()
        .expect("java project must have tool command");

    match (build_tool, spec.java_layout) {
        (EffectiveBuildTool::Maven, Some(JavaLayout::Single)) => {
            run_streamed(&CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args: vec![
                    "clean".to_string(),
                    "package".to_string(),
                    "-DskipTests".to_string(),
                ],
                display_override: None,
                workdir: Some(spec.project_root.clone()),
                envs: vec![],
                stdin_text: None,
            })
            .await?;
        }
        (EffectiveBuildTool::Gradle, Some(JavaLayout::Single)) => {
            run_streamed(&CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args: vec!["build".to_string(), "-x".to_string(), "test".to_string()],
                display_override: None,
                workdir: Some(spec.project_root.clone()),
                envs: vec![],
                stdin_text: None,
            })
            .await?;
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

            run_streamed(&CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args,
                display_override: None,
                workdir: Some(workdir),
                envs: vec![],
                stdin_text: None,
            })
            .await?;
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

            run_streamed(&CommandSpec {
                stage: "PackageJava",
                program: path_to_string(command),
                args,
                display_override: None,
                workdir: Some(workdir),
                envs: vec![],
                stdin_text: None,
            })
            .await?;
        }
        _ => {
            return Err(DeployError::ProjectMismatch {
                message: "Java 项目缺少可执行的构建布局信息".to_string(),
            }
            .into());
        }
    }

    let artifact_dir = spec
        .artifact_search_dir
        .as_ref()
        .context("Java 项目缺少产物搜索目录")?;
    let jar = find_executable_jar(artifact_dir)?;
    ui::print_stage_success("PackageJava", &format!("JAR 产物已生成：{}", jar.display()));
    Ok(jar)
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

    use super::find_executable_jar;

    #[test]
    fn choose_non_sources_jar() {
        let temp = TempDir::new().expect("tempdir");
        let dir = temp.path();
        fs::write(dir.join("demo-sources.jar"), b"small").expect("write");
        fs::write(dir.join("demo.jar"), b"larger-jar").expect("write");

        let jar = find_executable_jar(dir).expect("jar");
        assert_eq!(jar.file_name().and_then(|v| v.to_str()), Some("demo.jar"));
    }
}
