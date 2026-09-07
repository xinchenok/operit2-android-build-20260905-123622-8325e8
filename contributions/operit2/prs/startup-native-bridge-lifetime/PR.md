## 变更说明

**问题：** 退出或重启应用时销毁 native Runtime，可与尚未结束的 JNI 调用竞争导致进程崩溃。

**原因：** Executor.shutdownNow 不保证正在执行的 JNI 调用结束；terminateApplication 先 destroy 再退出。

**修复：** 该分支必定终止进程，因此先停止服务并完成既有退出流程，让进程退出统一回收 native bridge。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Android。模块：应用终止与重启。

修改文件：

- `apps/flutter/app/android/app/src/main/kotlin/app/operit/AndroidPlatformChannel.kt`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码核对 terminateApplication 后续的进程终止路径以及 AndroidRuntimeHost.destroy 的 shutdownNow/原生指针释放顺序。此补丁不改变常规请求执行。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 保持运行中的 native 请求。
2. 触发应用终止或重启。
3. 原代码 shutdownNow 后立即释放仍可能被调用使用的 bridge。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
