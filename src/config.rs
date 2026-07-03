use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::error::DeployError;

/// 项目内固定配置文件名。
pub const CONFIG_FILE_NAME: &str = ".deploy-sc.toml";

/// 项目配置根结构。
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub git: Option<GitConfig>,
    pub registry: Option<RegistryConfig>,
    pub k8s: Option<K8sConfig>,
}

/// Git 服务登录配置。
#[derive(Debug, Clone, Deserialize)]
pub struct GitConfig {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

/// Docker 仓库登录配置。
#[derive(Debug, Clone, Deserialize)]
pub struct RegistryConfig {
    pub server: String,
    pub username: String,
    pub password: Option<String>,
    pub token: Option<String>,
}

impl ProjectConfig {
    /// 从当前执行目录加载 `.deploy-sc.toml`。
    pub fn load(config_root: &Path) -> Result<Self> {
        let config_path = config_root.join(CONFIG_FILE_NAME);
        if !config_path.is_file() {
            return Err(DeployError::CredentialFileMissing { path: config_path }.into());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("读取配置文件失败：{}", config_path.display()))?;
        let parsed: ProjectConfig = toml::from_str(&content).context("TOML 解析失败")?;
        parsed.validate_syntax()?;
        Ok(parsed)
    }

    /// 校验所有配置的基本格式，但不强制每个阶段都必须存在。
    pub fn validate_syntax(&self) -> Result<()> {
        if let Some(git) = &self.git {
            git.validate()?;
        }

        if let Some(registry) = &self.registry {
            registry.validate()?;
        }

        if let Some(k8s) = &self.k8s {
            k8s.validate()?;
        }

        Ok(())
    }

    /// 在需要 Git 认证的阶段读取 Git 配置。
    pub fn git(&self) -> Result<&GitConfig> {
        self.git
            .as_ref()
            .ok_or(DeployError::MissingGitCredential.into())
    }

    /// 在 Docker 推送阶段读取仓库登录配置。
    pub fn registry(&self) -> Result<&RegistryConfig> {
        self.registry.as_ref().ok_or_else(|| {
            DeployError::CredentialFormat {
                message: "缺少 [registry] 配置".to_string(),
            }
            .into()
        })
    }

    /// 读取 K8s 部署配置，不存在时返回 None（跳过 K8s 阶段）。
    pub fn k8s(&self) -> Option<&K8sConfig> {
        self.k8s.as_ref()
    }
}

impl GitConfig {
    /// 校验 Git 配置必需字段。
    pub fn validate(&self) -> Result<()> {
        if self.base_url.trim().is_empty() {
            return Err(DeployError::CredentialFormat {
                message: "git.base_url 不能为空".to_string(),
            }
            .into());
        }

        if self.username.trim().is_empty() {
            return Err(DeployError::CredentialFormat {
                message: "git.username 不能为空".to_string(),
            }
            .into());
        }

        if self.password.trim().is_empty() {
            return Err(DeployError::CredentialFormat {
                message: "git.password 不能为空".to_string(),
            }
            .into());
        }

        Ok(())
    }
}

impl RegistryConfig {
    /// 校验 Docker 登录配置。
    pub fn validate(&self) -> Result<()> {
        if self.server.trim().is_empty() {
            return Err(DeployError::CredentialFormat {
                message: "registry.server 不能为空".to_string(),
            }
            .into());
        }

        if self.username.trim().is_empty() {
            return Err(DeployError::CredentialFormat {
                message: "registry.username 不能为空".to_string(),
            }
            .into());
        }

        let has_password = self.password.as_ref().is_some_and(|v| !v.trim().is_empty());
        let has_token = self.token.as_ref().is_some_and(|v| !v.trim().is_empty());

        if has_password == has_token {
            return Err(DeployError::CredentialFormat {
                message: "registry.password 与 registry.token 必须二选一".to_string(),
            }
            .into());
        }

        Ok(())
    }

    /// 返回 docker login 需要写入 stdin 的凭据内容。
    pub fn registry_secret(&self) -> &str {
        self.token
            .as_deref()
            .or(self.password.as_deref())
            .expect("validated config must contain either password or token")
    }
}

/// Kubernetes 部署配置。
///
/// 支持两种风格（二选一）：
/// - **顶层快捷方式**：直接设置 `kubeconfig` / `namespace` / `deployment` / `container`，用于单 Deployment 场景。
/// - **数组格式**：`[[k8s.deployments]]` 定义多个 Deployment 目标，每个可带 `module` 字段关联多模块项目子模块。
#[derive(Debug, Clone, Deserialize)]
pub struct K8sConfig {
    /// kubeconfig 文件路径。
    pub kubeconfig: Option<String>,
    /// k8s context 名称，可选，默认使用 kubeconfig 的当前 context。
    pub context: Option<String>,
    /// 命名空间（顶层快捷方式，与 deployments 互斥）。
    pub namespace: Option<String>,
    /// Deployment 名称（顶层快捷方式，与 deployments 互斥）。
    pub deployment: Option<String>,
    /// 容器名称（顶层快捷方式，可选，默认与 deployment 同名）。
    pub container: Option<String>,
    /// Deployment 目标列表（数组格式，与顶层 namespace/deployment 互斥）。
    #[serde(default)]
    pub deployments: Vec<K8sDeploymentTarget>,
}

/// 单个 K8s Deployment 部署目标。
#[derive(Debug, Clone, Deserialize)]
pub struct K8sDeploymentTarget {
    /// 关联的模块名，仅多模块 Java 项目使用。为空时匹配非模块项目。
    pub module: Option<String>,
    /// 命名空间。
    pub namespace: String,
    /// Deployment 名称。
    pub deployment: String,
    /// 容器名称，可选，默认与 deployment 同名。
    pub container: Option<String>,
}

impl K8sConfig {
    /// 校验 K8s 配置：至少提供一个部署目标。
    pub fn validate(&self) -> Result<()> {
        // 校验 kubeconfig
        if self.kubeconfig.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(DeployError::CredentialFormat {
                message: "k8s.kubeconfig 不能为空".to_string(),
            }
            .into());
        }

        let has_top_level = self.namespace.is_some() || self.deployment.is_some();
        let has_array = !self.deployments.is_empty();

        // 两种风格不能混用
        if has_top_level && has_array {
            return Err(DeployError::CredentialFormat {
                message: "k8s 配置不能同时使用顶层 namespace/deployment 与数组 deployments，请二选一"
                    .to_string(),
            }
            .into());
        }

        // 至少需要一种风格
        if !has_top_level && !has_array {
            return Err(DeployError::CredentialFormat {
                message: "k8s 配置至少需要一个部署目标：顶层 namespace/deployment 或数组 deployments"
                    .to_string(),
            }
            .into());
        }

        // 顶层风格：deployment 必填
        if has_top_level && self.deployment.as_ref().is_none_or(|s| s.trim().is_empty()) {
            return Err(DeployError::CredentialFormat {
                message: "k8s.deployment 不能为空".to_string(),
            }
            .into());
        }

        // 数组风格：每个条目校验
        for (i, target) in self.deployments.iter().enumerate() {
            if target.namespace.trim().is_empty() {
                return Err(DeployError::CredentialFormat {
                    message: format!("k8s.deployments[{i}].namespace 不能为空"),
                }
                .into());
            }
            if target.deployment.trim().is_empty() {
                return Err(DeployError::CredentialFormat {
                    message: format!("k8s.deployments[{i}].deployment 不能为空"),
                }
                .into());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectConfig;

    #[test]
    fn reject_invalid_secret_combination() {
        let config: ProjectConfig = toml::from_str(
            r#"
[registry]
server = "registry.example.com"
username = "ci"
password = "pwd"
token = "tok"
"#,
        )
        .expect("toml should parse");

        assert!(config.validate_syntax().is_err());
    }

    #[test]
    fn accept_git_and_registry_sections() {
        let config: ProjectConfig = toml::from_str(
            r#"
[git]
base_url = "https://git.example.com"
username = "git-user"
password = "git-password"

[registry]
server = "registry.example.com"
username = "ci"
password = "pwd"
"#,
        )
        .expect("toml should parse");

        assert!(config.validate_syntax().is_ok());
        assert_eq!(config.git().expect("git").username, "git-user");
        assert_eq!(
            config.registry().expect("registry").registry_secret(),
            "pwd"
        );
    }
}
