# deploy-sc

使用 Rust 编写的命令行自动化部署工具，覆盖以下阶段：

- Git 克隆 / 增量拉取
- 分支切换与远程跟踪分支创建
- Web / Java 项目识别
- Java 单模块 / 多模块打包
- Docker 镜像构建、登录、推送
- 版本号自动生成与最终结果回显

## 配置文件

在执行 `deploy-sc` 的当前目录放置 `.deploy-sc.toml`。该文件会在 `clone` 前读取，所以不能放在尚未拉取下来的目标仓库里。

```toml
[git]
base_url = "https://git.company.local"
username = "git-user"
password = "git-password"

[registry]
server = "registry.company.local"
username = "docker-user"
password = "docker-password"
```

Docker 仓库也可以使用 `token` 替代 `password`：

```toml
[git]
base_url = "https://git.company.local"
username = "git-user"
password = "git-password"

[registry]
server = "registry.company.local"
username = "docker-user"
token = "secret-token"
```

说明：

- Git 当前仅支持 `HTTPS 用户名 + 密码`
- `git.base_url` 要与 `--git-url` 或仓库的 `origin` 地址属于同一 Git 服务
- `registry.password` 与 `registry.token` 必须二选一

## 常用命令

Web 项目：

```bash
deploy-sc \
  --git-url https://example.com/demo/web.git \
  --branch master \
  --project-type web \
  --image registry.example.com/team/web-app \
  --mode clone \
  --force-clean
```

Java 单模块 Maven 项目：

```bash
deploy-sc \
  --git-url https://example.com/demo/service.git \
  --branch release \
  --project-type java \
  --java-layout single \
  --build-tool maven \
  --image registry.example.com/team/service
```

Java 多模块 Gradle 项目：

```bash
deploy-sc \
  --git-url https://example.com/demo/platform.git \
  --branch prod \
  --project-type java \
  --java-layout multi \
  --module module-a \
  --build-tool gradle \
  --image registry.example.com/team/module-a
```

## 说明

- 默认部署工作目录是当前目录下的 `.deploy-workspace/`
- 未传 `--tag` 时，版本号格式为 `分支-短提交哈希-时间戳`
- Docker 登录配置通过 `DOCKER_CONFIG=.deploy-workspace/.docker` 隔离到工作目录内
- Git clone / fetch / pull 会读取当前目录根配置中的 `[git]` 认证信息
- 长耗时命令会显示阶段日志和持续进度指示
