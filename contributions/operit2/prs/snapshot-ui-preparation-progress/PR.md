## 变更说明

**问题：** 选择大型快照后，文件打开、上传及检查阶段缺少清晰反馈，用户容易把仍在工作的准备流程当作卡死并重复操作。

**原因：** 准备过程散落在两个入口，上传器没有字节进度回调；导入进度流只覆盖提交阶段，不能解释此前的等待。

**修复：** 在打开文件选择器前先绘制共用准备对话框，分别展示打开、按字节读取、内容检查阶段，读取阶段节流更新进度，返回预览后才进入既有确认/提交流程。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Flutter 全平台。模块：引导和设置中的快照准备 UI、Dart 上传器可选进度回调。现有调用不传回调时保持兼容。

修改文件：

- `apps/flutter/app/lib/core/snapshot/SnapshotImportUploader.dart`
- `apps/flutter/app/lib/ui/common/Operit1SnapshotPreparationDialog.dart`
- `apps/flutter/app/lib/ui/features/onboarding/OnboardingStartupRoute.dart`
- `apps/flutter/app/lib/ui/features/settings/data/DataSettingsPanel.dart`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码检查首帧后打开 picker、100ms 节流字节回调、检查阶段不伪造百分比、Timer.dispose，以及关闭/失败时上传所有权。对话框仅调用原始 SnapshotImportFile.pick、stage、completeOperit1；无资源选择器或扩展导入 API。未运行视觉或设备验证。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 从引导或设置入口选择一个较大的 Operit1 ZIP 快照。
2. 在文档打开、分块传输和内容检查较慢时观察界面。
3. 原设置页在 picker 返回前没有 busy 状态，两个入口都没有区分文件读取与核心检查的进度。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
