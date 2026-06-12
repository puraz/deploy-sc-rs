# ============================================================
# 阶段一：编译 deploy-sc Rust 二进制
# ============================================================
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev perl

WORKDIR /build

# 先复制依赖清单，利用 Docker 层缓存加速
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release 2>&1 && \
    rm -rf src

COPY src/ ./src/
RUN touch src/main.rs && cargo build --release

# ============================================================
# 阶段二：可交付的运行镜像
# ============================================================
FROM eclipse-temurin:8-jdk

# 安装运行时依赖：git、Docker CLI、Maven
RUN apt-get update -qq && \
    apt-get install -y -qq --no-install-recommends \
        git \
        docker.io \
        maven \
        curl \
        unzip \
    && rm -rf /var/lib/apt/lists/*

# 安装 Gradle 7.6.3（最后一个支持 JDK 8 的版本）
ENV GRADLE_VERSION=7.6.3
RUN curl -fsSL "https://services.gradle.org/distributions/gradle-${GRADLE_VERSION}-bin.zip" \
        -o /tmp/gradle.zip && \
    unzip -q /tmp/gradle.zip -d /opt && \
    ln -s "/opt/gradle-${GRADLE_VERSION}/bin/gradle" /usr/local/bin/gradle && \
    rm /tmp/gradle.zip

# 复制 deploy-sc 二进制
COPY --from=builder /build/target/release/deploy-sc /usr/local/bin/deploy-sc

ENTRYPOINT ["deploy-sc"]
