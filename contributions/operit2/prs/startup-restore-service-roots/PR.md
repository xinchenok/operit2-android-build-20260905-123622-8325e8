## 变更说明

**问题：** Android 在 Flutter 尚未重新配置运行时时重建前台服务，会因缺失存储根目录崩溃；清除数据后的服务启动也会进入同一失败路径。

**原因：** 存储目录只保存在 AndroidRuntimeHost 的进程内字段中；ensureRuntimeHandle 直接使用这些字段，没有从已有 bootstrap 配置恢复。

**修复：** 启动核心前读取并验证已确认的 bootstrap 配置和 activeIdentityId，再按既有 identities 目录规则设置根目录。未确认存储时服务停止，等待正常引导；主线程检查已发布状态，无需持有初始化锁。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Android。模块：前台服务、运行时启动与所选身份恢复。

修改文件：

- `apps/flutter/app/android/app/src/main/kotlin/app/operit/AndroidRuntimeHost.kt`
- `apps/flutter/app/android/app/src/main/kotlin/app/operit/OperitCoreService.kt`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码核对了 RuntimeBootstrapModels 的 confirmed/identities/activeIdentityId 字段，以及 RuntimeBootstrapManager 的身份目录拼接规则。服务的后台恢复和主线程 stopSelf 分支保持既有线程边界。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 完成初始化并切换到非默认身份。
2. 让系统回收进程，然后在 Flutter 页面创建前重建服务。
3. 原代码无法取得所选身份目录；清空应用数据后的同类启动也失败。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
