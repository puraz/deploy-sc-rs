#!/usr/bin/env bash
#
# deploy-sc 容器化包装脚本
# =============================
# 在 Docker 容器内运行 deploy-sc，宿主机只需安装 Docker。
# 自动构建镜像，挂载 Docker socket 和当前目录，透传所有参数。
#
# 用法：
#   ./scripts/deploy.sh [deploy-sc 参数...]
#
# 首次使用会自动构建镜像，之后直接运行。
# 如需强制重新构建：BUILD=1 ./scripts/deploy.sh ...
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

IMAGE_NAME="deploy-sc-toolchain"
DOCKERFILE_DIR="$PROJECT_DIR"

# 构建镜像（首次自动构建，也可通过 BUILD=1 强制重构建）
if [ ! -z "${BUILD:-}" ] || ! docker image inspect "$IMAGE_NAME" &>/dev/null; then
    echo "==> 构建工具链镜像: $IMAGE_NAME ..."
    docker build -t "$IMAGE_NAME" "$DOCKERFILE_DIR"
    echo "==> 构建完成"
fi

# 挂载 /var/run/docker.sock → 容器内 docker build/push 复用宿主机 Docker daemon
# 挂载 $PWD 到容器内相同路径 → Docker daemon 能直接访问构建上下文文件
exec docker run --rm \
    -v /var/run/docker.sock:/var/run/docker.sock \
    -v "$PWD:$PWD" \
    -w "$PWD" \
    "$IMAGE_NAME" \
    "$@"
