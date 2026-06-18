### 部署脚本

1.供应商后台
cargo run --   --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
--branch master \
--project-type java \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-enterprise-admin \
--mode auto \
--module supply-enterprise-admin \
--java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn


2.运营商后台
cargo run --   --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
--branch master \
--project-type java \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/enterprise-admin \
--mode auto \
--module sellretail-enterprise-admin \
--java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn

3.前端集采商城
cargo run --   --git-url http://119.3.246.2:10082/WEB/pocket-donkey-purchase.git \
--branch release \
--project-type web \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchase-h5 \
--mode auto

4.集采商城接口
cargo run --   --git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
--branch master \
--project-type java \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchaseuser-api \
--mode auto \
--module supply-purchaseuser-api \
--java-home=/Users/puraz/.sdkman/candidates/java/8.0.472-amzn


-- 测试环境服务器
./deploy-sc \
--git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
--branch master \
--project-type java \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-enterprise-admin \
--mode auto \
--module supply-enterprise-admin \
--java-home=/usr/local/jdk1.8.0_201

./deploy-sc \
--git-url http://119.3.246.2:10082/JAVA/sellRetail.git \
--branch master \
--project-type java \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/enterprise-admin \
--mode auto \
--module sellretail-enterprise-admin \
--java-home=/usr/local/jdk1.8.0_201

./deploy-sc \
--git-url http://119.3.246.2:10082/WEB/pocket-donkey-purchase.git \
--branch release \
--project-type web \
--image registry.cn-guangzhou.aliyuncs.com/sell_prod/supply-purchase-h5 \
--mode auto
