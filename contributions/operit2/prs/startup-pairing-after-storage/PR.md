## 变更说明

**问题：** 首次启动尚未确认存储时即订阅 native 配对事件，可能过早触发核心初始化并使配对流失效。

**原因：** 配对事件订阅只跟随 Widget 初始化，不观察 RuntimeBootstrapManager.runtimeConfigured。

**修复：** 监听 bootstrap 状态，仅在存储已配置后建立事件订阅；配置撤销或组件释放时取消，流结束后释放订阅标记。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：Flutter 的非 Web host。模块：Web Access 配对事件监听。

修改文件：

- `apps/flutter/app/lib/ui/main/OperitApp.dart`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码确认 OperitApp 已引用 RuntimeBootstrapManager，runtimeConfigured 是原始 API；新增监听的添加/移除与订阅清理匹配。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。未创建提交，DCO 签署仍须由最终提交者完成。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 在没有已确认存储配置的安装中打开应用。
2. 完成引导中的存储确认。
3. 原配对订阅只在 initState 建立一次，早期失败后不会随配置完成重新建立。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。
- [ ] 我拥有本次提交内容的版权或必要授权。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
