## 变更说明

**问题：** 删除暂存上传或关闭输入流失败会覆盖原始读取/导入错误，甚至把已成功的导入显示为失败。上传创建本身失败时还会漏关已选文件。

**原因：** 清理操作与业务操作共用可抛错的 await 链，上传创建位于资源释放的 try/finally 之外。

**修复：** 将上传创建纳入 finally 资源管理，清理错误独立记录并保留原始异常堆栈；两个导入入口和页面释放复用安全暂存清理函数，成功结果不因辅助清理失败变更。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Flutter 全平台。模块：快照输入流、暂存上传及两个导入入口。

修改文件：

- `apps/flutter/app/lib/core/snapshot/SnapshotImportUploader.dart`
- `apps/flutter/app/lib/ui/features/onboarding/OnboardingStartupRoute.dart`
- `apps/flutter/app/lib/ui/features/settings/data/DataSettingsPanel.dart`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码逐条检查 begin/write/complete 的异常路径以及 finally：输入流始终尝试关闭；discard 失败被独立记录；Error.throwWithStackTrace 使用原始错误和堆栈。使用原始暂存 API，无结果对话框依赖。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。本 PR 对应单独提交并包含 DCO Signed-off-by。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 使快照导入成功，随后让 discardArchiveUpload 报错；原 UI 会显示“导入失败”。
2. 或使上传读取失败且清理也失败，原始原因被清理异常覆盖。
3. beginArchiveUpload 失败时原 try/finally 尚未进入，输入流没有关闭。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
