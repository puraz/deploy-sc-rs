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
