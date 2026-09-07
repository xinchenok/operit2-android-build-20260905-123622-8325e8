## 变更说明

### 问题

已轮询并回收退出状态的终端在析构时仍向旧 PID 发送信号；PID 被系统复用后，清理会指向已不属于该会话的进程。

### 复现步骤

1. 启动终端并让 shell 退出。
2. 通过 pollPtyExitCode 回收其退出状态。
3. 保留会话对象一段时间后关闭；原实现仍在 Drop 中无条件 kill 旧 PID。

以上是根据现有控制流整理的复现路径，本次未在 Android 设备上执行。

### 原因

pollPidExitCode 使用 waitpid 回收子进程并把状态存入 exitCode，Drop 没有读取该所有权状态。

### 修复

只有 exitCode 尚未记录时才发送 SIGHUP/SIGKILL；文件描述符依然按照原有路径清理。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：Android terminal host。
- 行为与兼容性：已退出会话不会再次向旧 PID 发信号；仍在运行的会话保持原有终止行为。
- host API 与 Flutter/Rust 边界：无接口、配置或数据库迁移。

## 验证

- 已读取上游 CONTRIBUTING.md 与 PR 模板，并逐处对照固定基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的控制流和已有 API。
- 补丁生成时用精确、唯一匹配断言检查每处旧代码；每个 PR 独立从该上游基线生成完整文件，没有把其他 PR 当作前置分支。
- 已将受影响文件从固定基线导出到临时目录，执行 `git apply --check fix.patch`，独立应用检查通过。
- 本次按要求只做源码检查，未运行 cargo、Gradle、Flutter 构建、测试任务或 Android 设备复现；行为验证仍待维护者执行。
- 未添加公共接口或操作说明；本 PR 的复现路径记录了需要补做的设备验证，未新增无法在本次环境验证的设备测试。



## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1（由实际提交者提交时确认）。
- [ ] 我拥有本次提交内容的版权或必要授权（由实际提交者确认）。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。
