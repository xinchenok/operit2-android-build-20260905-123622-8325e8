## 变更说明

### 问题

PRoot 启动环境未设置兼容开关；在无法使用其 seccomp 加速路径的 Android 环境中，Linux 终端及启动探测可能失败。

### 复现步骤

1. 在会拒绝 PRoot seccomp 路径的 Android 设备上安装运行时。
2. 打开 bash 终端，或调用生成的 `login_ubuntu` 启动脚本。
3. 检查探测/启动失败输出与 `PROOT_NO_SECCOMP` 环境值。

以上是根据现有控制流整理的复现路径，本次未在 Android 设备上执行。

### 原因

Rust PTY 的显式 envp 和 Kotlin 生成的公共 shell 环境都设置了 PROOT_LOADER，但均未设置 PROOT_NO_SECCOMP，两个入口会继续走不兼容路径。

### 修复

在两个已有 PRoot 环境构造位置加入 `PROOT_NO_SECCOMP=1`，让探测与实际启动使用一致的兼容模式。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：Android terminal host 与 AndroidRuntimeAssets 生成的 PRoot 启动脚本。
- 行为与兼容性：仅改变 PRoot 的启动模式，可能放弃其 seccomp 加速；不修改系统 shell 或 guest 命令环境协议。
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
