## 变更说明

**问题：** 核心已返回跳过字段和实际迁移计数，但 UI 仅提示成功并进入应用，用户无法得知哪些内容需要手工处理。

**原因：** 两个导入入口都只使用返回值写日志，没有向用户呈现已有的迁移结果。

**修复：** 两个入口共用结果对话框，展示上游已提供的计数和全部非空迁移说明；引导文字说明需选原始快照以及快照外数据的边界。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Flutter 全平台。模块：Operit1 快照结果展示与引导说明。

修改文件：

- `apps/flutter/app/lib/ui/common/Operit1ImportResultDialog.dart`
- `apps/flutter/app/lib/ui/features/onboarding/OnboardingStartupRoute.dart`
- `apps/flutter/app/lib/ui/features/settings/data/DataSettingsPanel.dart`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码核对 Operit1SnapshotImportManager.rs 中原始结果的计数及 skippedFields 字段。去掉增强版资源选择器、扩展导入 API 和复制按钮，不对迁移说明按标点过滤。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 导入一个核心会记录 modelConfig.skippedFields 的 Operit1 快照。
2. 等待导入成功。
3. 原引导直接完成，设置页只显示通用成功提示，迁移说明未展示。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
