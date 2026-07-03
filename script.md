# 部署脚本参考

## Git 仓库信息

- **Git URL**: `http://119.3.246.2:10082/JAVA/sellRetail.git`
- **分支**: `master`
- **项目类型**: `java`
- **JDK 版本**: `1.8`

## Java 多模块对应关系一览

| # | 中文名称 | Maven Module | Docker 镜像名 | K8s Deployment |
|---|---------|-------------|--------------|----------------|
| 1 | 总后台 | base-admin | admin | admin |
| 2 | 运营商后台 | sellretail-enterprise-admin | enterprise-admin | enterprise-admin |
| 3 | 供应商后台 | supply-enterprise-admin | supply-enterprise-admin | supply-enterprise-admin |
| 4 | 门店后台 | sellretail-shop-admin | shop-admin | shop-admin |
| 5 | 数据大屏 | sellretail-screen | sell-screen | sell-screen |
| 6 | 集采商城接口 | supply-purchaseuser-api | supply-purchaseuser-api | supply-purchaseuser-api |
| 7 | 零售端接口 | sellretail-appuser-api | appuser-api | appuser-api |
| 8 | 分拣中心 app 接口 | supply-pickupcenter-api | supply-pickupcenter-api | supply-pickupcenter-api |
| 9 | 供应商 app 接口 | supply-enterpriseapp-api | supply-enterpriseapp-api | supply-enterpriseapp-api |
| 10 | 电子秤接口 | supply-electronicscale-api | supply-electronicscale-api | supply-electronicscale-api |
| 11 | 门店助手接口 | sellretail-shop-api | shop-api | shop-api |

---

## 本地部署（cargo run --）

### 1. 总后台
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/admin \
  --mode auto \
  --module base-admin \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 2. 运营商后台
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/enterprise-admin \
  --mode auto \
  --module sellretail-enterprise-admin \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 3. 供应商后台
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-enterprise-admin \
  --mode auto \
  --module supply-enterprise-admin \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 4. 门店后台
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/shop-admin \
  --mode auto \
  --module sellretail-shop-admin \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 5. 数据大屏
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/sell-screen \
  --mode auto \
  --module sellretail-screen \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 6. 集采商城接口
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchaseuser-api \
  --mode auto \
  --module supply-purchaseuser-api \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 7. 零售端接口
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/appuser-api \
  --mode auto \
  --module sellretail-appuser-api \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 8. 分拣中心 app 接口
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-pickupcenter-api \
  --mode auto \
  --module supply-pickupcenter-api \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 9. 供应商 app 接口
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-enterpriseapp-api \
  --mode auto \
  --module supply-enterpriseapp-api \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 10. 电子秤接口
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-electronicscale-api \
  --mode auto \
  --module supply-electronicscale-api \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

### 11. 门店助手接口
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/shop-api \
  --mode auto \
  --module sellretail-shop-api \
  --java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn
```

---

## 服务器部署（./deploy-sc）

> 服务器 JDK 路径：`/usr/local/jdk1.8.0_201`

### 1. 总后台
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/admin \
  --mode auto \
  --module base-admin \
  --java-home=/usr/local/jdk1.8.0_201
```

### 2. 运营商后台
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/enterprise-admin \
  --mode auto \
  --module sellretail-enterprise-admin \
  --java-home=/usr/local/jdk1.8.0_201
```

### 3. 供应商后台
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-enterprise-admin \
  --mode auto \
  --module supply-enterprise-admin \
  --java-home=/usr/local/jdk1.8.0_201
```

### 4. 门店后台
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/shop-admin \
  --mode auto \
  --module sellretail-shop-admin \
  --java-home=/usr/local/jdk1.8.0_201
```

### 5. 数据大屏
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/sell-screen \
  --mode auto \
  --module sellretail-screen \
  --java-home=/usr/local/jdk1.8.0_201
```

### 6. 集采商城接口
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchaseuser-api \
  --mode auto \
  --module supply-purchaseuser-api \
  --java-home=/usr/local/jdk1.8.0_201
```

### 7. 零售端接口
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/appuser-api \
  --mode auto \
  --module sellretail-appuser-api \
  --java-home=/usr/local/jdk1.8.0_201
```

### 8. 分拣中心 app 接口
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-pickupcenter-api \
  --mode auto \
  --module supply-pickupcenter-api \
  --java-home=/usr/local/jdk1.8.0_201
```

### 9. 供应商 app 接口
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-enterpriseapp-api \
  --mode auto \
  --module supply-enterpriseapp-api \
  --java-home=/usr/local/jdk1.8.0_201
```

### 10. 电子秤接口
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-electronicscale-api \
  --mode auto \
  --module supply-electronicscale-api \
  --java-home=/usr/local/jdk1.8.0_201
```

### 11. 门店助手接口
```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
  --branch master \
  --project-type java \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/shop-api \
  --mode auto \
  --module sellretail-shop-api \
  --java-home=/usr/local/jdk1.8.0_201
```

---

## 前端项目参考

### 集采商城 H5
```bash
cargo run -- \
  --git-url http://119.3.246.2:10082/WEB/pocket-donkey-purchase.git \
  --branch release \
  --project-type web \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchase-h5 \
  --mode auto
```

```bash
./deploy-sc \
  --git-url http://119.3.246.2:10082/WEB/pocket-donkey-purchase.git \
  --branch release \
  --project-type web \
  --image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchase-h5 \
  --mode auto
```
