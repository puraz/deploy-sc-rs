# deploy-sc

使用 Rust 编写的命令行自动化部署工具，覆盖以下阶段：

- Git 克隆 / 增量拉取
- 分支切换与远程跟踪分支创建
- Web / Java 项目识别
- Java 单模块 / 多模块打包
- Docker 镜像构建、登录、推送
- 版本号自动生成与最终结果回显

## 快速开始（容器化运行）

宿主机只需安装 **Docker**，无需本地安装 Git、JDK、Maven、Gradle。

首次使用自动构建工具链镜像，后续直接运行：

```bash
# 任意目录，只需当前目录有 .deploy-sc.toml 配置
./scripts/deploy.sh \
  --git-url https://example.com/demo/web.git \
  --branch master \
  --project-type web \
  --image registry.example.com/team/web-app
```

强制重新构建镜像：

```bash
BUILD=1 ./scripts/deploy.sh --help
```

### 工作原理

`scripts/deploy.sh` 内部执行 `docker run --rm`：

- **挂载 `/var/run/docker.sock`** — 容器内的 `docker build`/`push` 复用宿主机的 Docker daemon
- **挂载 `$PWD` 到容器内相同路径** — Docker daemon 能直接访问构建上下文文件
- **工作目录设为 `$PWD`** — 相对路径 `.deploy-sc.toml`、`.deploy-workspace/` 均按预期工作
- **镜像内预装** Git、JDK 8、Maven 3.9、Gradle 7.6、Docker CLI

## 传统运行方式（本地依赖）

宿主机需安装：Rust（仅编译期）、Git、JDK 8、Maven/Gradle、Docker。

```bash
cargo run -- ...
```

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

- Git 当前支持 `HTTP/HTTPS 用户名 + 密码`
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
  --java-home /path/to/jdk8 \
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
  --java-home /path/to/jdk8 \
  --image registry.example.com/team/module-a
```

## 说明

- 默认部署工作区根目录是当前目录下的 `.deploy-workspace/`
- 每个仓库会按 `host+path` 生成独立子目录，例如 `.deploy-workspace/git_example_com_team_app/`
- `clone` / `pull` / `auto` 模式都需要传入 `--git-url`，用于定位对应项目子目录
- Java 项目如需固定使用特定 JDK，可通过 `--java-home` 指定，例如老项目传 JDK 8 根目录
- 未传 `--tag` 时，版本号格式为 `分支-短提交哈希-时间戳`
- Docker 登录配置通过 `DOCKER_CONFIG=.deploy-workspace/<repo-key>/.docker` 隔离到项目目录内
- Git clone / fetch / pull 会读取当前目录根配置中的 `[git]` 认证信息
- 长耗时命令会显示阶段日志和持续进度指示
