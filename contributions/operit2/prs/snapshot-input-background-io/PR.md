## 变更说明

**问题：** 大型快照或慢速文档提供方会让选文件后的页面停顿，并在分块读取期间反复阻塞 Android 主线程。

**原因：** query/openInputStream/read/close 与 Flutter 通道处理运行在同一主线程。

**修复：** 用一个专属串行执行器拥有输入流和 token 注册表，后台完成元数据查询、打开、读取及关闭，再把结果投递回主线程；通道释放排队关闭流而不等待 I/O。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Android。模块：快照文档输入通道。保留原有 ZIP MIME 类型与 64 KiB 分块上限。

修改文件：

- `apps/flutter/app/android/app/src/main/kotlin/app/operit/SnapshotImportInputChannel.kt`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码检查全部 openInputs 访问均归属同一工作线程；clear 不同步等待流关闭，待处理 picker 仅完成一次。手动适配原始 ZIP-only pickSnapshot 签名，没有引入 allowJson。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 从速度较慢的 DocumentProvider 选择快照 ZIP。
2. 等待元数据查询、打开及分块读取。
3. 原 Activity 回调和 MethodChannel 回调在主线程同步执行这些 I/O。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
