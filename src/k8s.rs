use std::path::Path;

use anyhow::{Context, Result};
use k8s_openapi::api::apps::v1::Deployment;
use kube::{
    Client,
    api::{Api, Patch, PatchParams},
    config::{KubeConfigOptions, Kubeconfig},
};
use tokio::time::{Duration, sleep};

use crate::{
    config::{K8sConfig, K8sDeploymentTarget, ProjectConfig},
    context::{ImageMetadata, ProjectSpec, RunContext},
    error::DeployError,
    ui,
};

/// K8s 部署阶段的主入口。
///
/// 根据配置解析部署目标列表，逐个执行 Deployment 镜像更新并等待 Rollout 就绪。
pub async fn trigger_deployment(
    ctx: &RunContext,
    project_config: &ProjectConfig,
    project_spec: &ProjectSpec,
    image: &ImageMetadata,
) -> Result<()> {
    let k8s_config = match project_config.k8s() {
        Some(cfg) => cfg,
        None => {
            ui::print_info("K8sDeploy", "未配置 [k8s]，跳过 K8s 部署");
            return Ok(());
        }
    };

    if ctx.cli.skip_k8s {
        ui::print_info("K8sDeploy", "--skip-k8s 已设置，跳过 K8s 部署");
        return Ok(());
    }

    let targets = resolve_targets(k8s_config, project_spec);
    if targets.is_empty() {
        ui::print_info("K8sDeploy", "未匹配到部署目标，跳过 K8s 部署");
        return Ok(());
    }

    let kubeconfig_path = k8s_config
        .kubeconfig
        .as_deref()
        .context("k8s.kubeconfig 未配置")?;
    let kubeconfig_path = Path::new(kubeconfig_path);

    ui::print_stage_start("K8sDeploy", "开始加载 K8s 集群配置");

    let client = build_client(k8s_config, kubeconfig_path).await?;

    ui::print_stage_success("K8sDeploy", "K8s 客户端初始化成功");

    let timeout = Duration::from_secs(ctx.cli.k8s_timeout);

    for target in &targets {
        let container_name = target
            .container
            .as_deref()
            .unwrap_or(&target.deployment);

        ui::print_info(
            "K8sDeploy",
            &format!(
                "更新 Deployment: {}/{}，容器: {}，镜像: {}",
                target.namespace, target.deployment, container_name, image.full_name
            ),
        );

        patch_deployment(&client, target, container_name, &image.full_name).await?;

        ui::print_info(
            "K8sDeploy",
            &format!(
                "等待 Rollout 就绪: {}/{}（超时 {}s）",
                target.namespace,
                target.deployment,
                ctx.cli.k8s_timeout
            ),
        );

        watch_rollout(&client, target, timeout).await?;

        ui::print_stage_success(
            "K8sDeploy",
            &format!(
                "Deployment {}/{} 更新完成",
                target.namespace, target.deployment
            ),
        );
    }

    ui::print_stage_success(
        "K8sDeploy",
        &format!("K8s 部署完成，共更新 {} 个 Deployment", targets.len()),
    );

    Ok(())
}

/// 根据配置和项目信息解析需要更新的 Deployment 目标列表。
///
/// 匹配规则：
/// - 多模块项目（指定了 --module）：匹配 `deployments[].module == cli.module`
/// - 非模块项目：匹配 `deployments[]` 中 `module` 为空的条目，或使用顶层快捷方式
fn resolve_targets(
    config: &K8sConfig,
    project_spec: &ProjectSpec,
) -> Vec<K8sDeploymentTarget> {
    // 数组风格：从 deployments[] 中筛选
    if !config.deployments.is_empty() {
        let is_multi_module = project_spec.module_name.is_some();
        return config
            .deployments
            .iter()
            .filter(|target| {
                if is_multi_module {
                    target
                        .module
                        .as_deref()
                        .is_some_and(|m| m == project_spec.module_name.as_deref().unwrap())
                } else {
                    target.module.is_none()
                }
            })
            .cloned()
            .collect();
    }

    // 顶层快捷风格：使用 namespace / deployment / container
    let mut targets = Vec::new();
    if let (Some(namespace), Some(deployment)) = (&config.namespace, &config.deployment) {
        targets.push(K8sDeploymentTarget {
            module: None,
            namespace: namespace.clone(),
            deployment: deployment.clone(),
            container: config.container.clone(),
        });
    }
    targets
}

/// 从 kubeconfig 文件构建 K8s 客户端。
async fn build_client(config: &K8sConfig, kubeconfig_path: &Path) -> Result<Client> {
    if !kubeconfig_path.is_file() {
        return Err(DeployError::K8sConfigError {
            message: format!("kubeconfig 文件不存在：{}", kubeconfig_path.display()),
        }
        .into());
    }

    let mut kubeconfig = Kubeconfig::read_from(kubeconfig_path)
        .with_context(|| format!("读取 kubeconfig 失败：{}", kubeconfig_path.display()))?;

    // 如果配置中指定了 context，覆盖 kubeconfig 的当前 context
    if let Some(context) = &config.context {
        kubeconfig.current_context = Some(context.clone());
    }

    let kube_config = kube::Config::from_custom_kubeconfig(
        kubeconfig,
        &KubeConfigOptions::default(),
    )
    .await
    .context("解析 kubeconfig 失败，请检查文件格式和集群可达性")?;

    let client = Client::try_from(kube_config)
        .context("创建 K8s 客户端失败")?;

    Ok(client)
}

/// 通过 JSON Merge Patch 更新 Deployment 的容器镜像。
async fn patch_deployment(
    client: &Client,
    target: &K8sDeploymentTarget,
    container_name: &str,
    image_full_name: &str,
) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), &target.namespace);

    let patch = serde_json::json!({
        "spec": {
            "template": {
                "spec": {
                    "containers": [
                        {
                            "name": container_name,
                            "image": image_full_name,
                        }
                    ]
                }
            }
        }
    });

    let params = PatchParams::default();
    api.patch(&target.deployment, &params, &Patch::Merge(&patch))
        .await
        .with_context(|| {
            format!(
                "PATCH Deployment 失败：{}/{}，容器：{}",
                target.namespace, target.deployment, container_name
            )
        })?;

    Ok(())
}

/// 轮询等待 Deployment Rollout 就绪。
///
/// 就绪条件（参考 kubectl rollout status）：
/// 1. `status.observedGeneration >= metadata.generation`
/// 2. `status.updatedReplicas >= spec.replicas`
/// 3. `status.availableReplicas >= spec.replicas`
async fn watch_rollout(client: &Client, target: &K8sDeploymentTarget, timeout: Duration) -> Result<()> {
    let api: Api<Deployment> = Api::namespaced(client.clone(), &target.namespace);
    let deadline = tokio::time::Instant::now() + timeout;
    let poll_interval = Duration::from_secs(5);

    while tokio::time::Instant::now() < deadline {
        let deployment = api.get(&target.deployment).await.with_context(|| {
            format!(
                "获取 Deployment 状态失败：{}/{}",
                target.namespace, target.deployment
            )
        })?;

        let status = match deployment.status {
            Some(ref s) => s,
            None => {
                sleep(poll_interval).await;
                continue;
            }
        };

        let spec = match deployment.spec {
            Some(ref s) => s,
            None => {
                sleep(poll_interval).await;
                continue;
            }
        };

        let generation = deployment.metadata.generation.unwrap_or(0);
        let observed = status.observed_generation.unwrap_or(0);
        let replicas = spec.replicas.unwrap_or(1);
        let updated = status.updated_replicas.unwrap_or(0);
        let available = status.available_replicas.unwrap_or(0);

        if observed >= generation && updated >= replicas && available >= replicas {
            return Ok(());
        }

        ui::print_info(
            "K8sDeploy",
            &format!(
                "Rollout 进行中: {}/{}  updated={}/{}  available={}/{}",
                target.namespace, target.deployment, updated, replicas, available, replicas
            ),
        );

        sleep(poll_interval).await;
    }

    Err(DeployError::K8sRolloutTimeout {
        namespace: target.namespace.clone(),
        deployment: target.deployment.clone(),
        timeout: timeout.as_secs(),
    }
    .into())
}

#[cfg(test)]
mod tests {
    use crate::{
        cli::{JavaLayout, ProjectType},
        config::{K8sConfig, K8sDeploymentTarget},
        context::ProjectSpec,
    };

    use super::resolve_targets;

    fn make_deployments() -> K8sConfig {
        K8sConfig {
            kubeconfig: Some("/tmp/kubeconfig".to_string()),
            context: None,
            namespace: None,
            deployment: None,
            container: None,
            deployments: vec![
                K8sDeploymentTarget {
                    module: Some("module-a".to_string()),
                    namespace: "prod".to_string(),
                    deployment: "svc-a".to_string(),
                    container: None,
                },
                K8sDeploymentTarget {
                    module: Some("module-b".to_string()),
                    namespace: "prod".to_string(),
                    deployment: "svc-b".to_string(),
                    container: Some("app".to_string()),
                },
                K8sDeploymentTarget {
                    module: None,
                    namespace: "staging".to_string(),
                    deployment: "default-svc".to_string(),
                    container: None,
                },
            ],
        }
    }

    fn make_project_spec(module: Option<&str>) -> ProjectSpec {
        ProjectSpec {
            project_type: ProjectType::Java,
            java_layout: if module.is_some() {
                Some(JavaLayout::Multi)
            } else {
                Some(JavaLayout::Single)
            },
            module_name: module.map(|s| s.to_string()),
            build_tool: Some(crate::context::EffectiveBuildTool::Maven),
            build_tool_command: None,
            project_root: "/tmp/repo".into(),
            dockerfile_path: "/tmp/repo/Dockerfile".into(),
            build_context_dir: "/tmp/repo".into(),
            artifact_search_dir: None,
        }
    }

    #[test]
    fn resolve_module_a_targets() {
        let config = make_deployments();
        let spec = make_project_spec(Some("module-a"));
        let targets = resolve_targets(&config, &spec);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].deployment, "svc-a");
    }

    #[test]
    fn resolve_module_b_targets() {
        let config = make_deployments();
        let spec = make_project_spec(Some("module-b"));
        let targets = resolve_targets(&config, &spec);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].deployment, "svc-b");
        assert_eq!(targets[0].container.as_deref(), Some("app"));
    }

    #[test]
    fn resolve_non_module_targets() {
        let config = make_deployments();
        let spec = make_project_spec(None);
        let targets = resolve_targets(&config, &spec);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].deployment, "default-svc");
    }

    #[test]
    fn resolve_unknown_module_returns_empty() {
        let config = make_deployments();
        let spec = make_project_spec(Some("unknown-module"));
        let targets = resolve_targets(&config, &spec);
        assert!(targets.is_empty());
    }
}
