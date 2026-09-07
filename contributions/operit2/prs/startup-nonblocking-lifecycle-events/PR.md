## 变更说明

**问题：** 冷启动期间 Android 生命周期事件会等待资源准备及 native 核心初始化，阻塞主线程。

**原因：** emitRuntimeEvent 与耗时的 ensureRuntimeHandle 共用 runtimeLock。

**修复：** 给事件队列独立短锁；创建 native handle 后在事件锁内发布并排空队列，使生命周期入队不等待运行时初始化。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Android。模块：生命周期事件转发。

修改文件：

- `apps/flutter/app/android/app/src/main/kotlin/app/operit/AndroidRuntimeHost.kt`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码核对了 pendingRuntimeEvents 的所有读写以及 runtimeHandle 的发布点：待发事件排空与并发新事件均受 runtimeEventLock 保护；native 创建仍由 runtimeLock 串行化。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 在资源准备或 native 初始化耗时较长的设备上冷启动。
2. 此时触发 Activity/Service 生命周期事件。
3. 原 emitRuntimeEvent 等待 runtimeLock，界面可能无响应。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
