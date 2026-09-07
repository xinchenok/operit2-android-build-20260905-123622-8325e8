## 变更说明

**问题：** 设置页在导入确认对话框等待期间释放后，确认返回仍执行 setState 和提交导入。

**原因：** await 确认对话框后只检查 confirmed，没有重新验证 mounted。

**修复：** 确认后同时检查 mounted；页面已释放时释放暂存文件并返回。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Flutter 全平台。模块：设置中的 Operit1 快照导入。

修改文件：

- `apps/flutter/app/lib/ui/features/settings/data/DataSettingsPanel.dart`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码核对 await _Operit1SnapshotImportDialog.show 与紧接着的 setState：新生命周期检查位于它们之间；保留原始 session.discard API。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 在设置页选择快照并打开导入确认框。
2. 使设置页在确认完成前退出或被替换。
3. 确认返回 true 时原代码在已释放状态上继续 setState。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
