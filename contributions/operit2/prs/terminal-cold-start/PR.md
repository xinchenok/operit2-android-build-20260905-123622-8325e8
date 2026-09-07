## 变更说明

### 问题

首次启动 Linux 终端时，逐目录启动 PRoot 探测可能累计超过 10 秒 prompt 截止时间，导致可用运行时被当作启动失败。

### 复现步骤

1. 在首次解包或缓存较冷的 Android 运行时中打开 bash。
2. 检查每个可访问 bind 目录触发的独立 PRoot probe。
3. 探测总耗时超过 10 秒时，PTY 等待先结束并关闭尚在初始化的进程。

以上是根据现有控制流整理的复现路径，本次未在 Android 设备上执行。

### 原因

login_ubuntu 对整个 bind 列表逐个启动 Linux probe，启动成本随可访问目录数累积；Rust 端只提供 10 秒初始 prompt 等待。

### 修复

先收集通过既有可访问性检查的 bind，一次探测完整集合；只有集合探测失败才回退原有逐目录排除流程。同时把首次 prompt 等待调整为 30 秒，容纳冷启动及兼容回退。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：Android terminal host 与 AndroidRuntimeAssets 生成的 PRoot 启动脚本。
- 行为与兼容性：保持既有 bind 可访问性判断和逐目录兼容降级；正常情况下减少 probe 次数，启动失败的最大等待时间由 10 秒调整为 30 秒。
- host API 与 Flutter/Rust 边界：无接口、配置或数据库迁移。

## 验证

- 已读取上游 CONTRIBUTING.md 与 PR 模板，并逐处对照固定基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的控制流和已有 API。
- 补丁生成时用精确、唯一匹配断言检查每处旧代码；每个 PR 独立从该上游基线生成完整文件，没有把其他 PR 当作前置分支。
- 已将受影响文件从固定基线导出到临时目录，执行 `git apply --check fix.patch`，独立应用检查通过。
- 本次按要求只做源码检查，未运行 cargo、Gradle、Flutter 构建、测试任务或 Android 设备复现；行为验证仍待维护者执行。
- 未添加公共接口或操作说明；本 PR 的复现路径记录了需要补做的设备验证，未新增无法在本次环境验证的设备测试。



## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
