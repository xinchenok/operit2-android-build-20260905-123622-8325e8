## 变更说明

**问题：** native watch 打开失败时，调用方可能永久等待而收不到原始错误。

**原因：** 单订阅 StreamController 尚无监听者时 close 的 Future 可等待监听端消费 done，而打开失败路径正等待这个 Future 才能继续。

**修复：** 仍向控制器投递原始错误并关闭，但不等待 close Future，允许打开失败正常向调用方传播。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

平台：使用 MethodChannelCoreProxy 的 Flutter host。模块：watch 错误处理。

修改文件：

- `apps/flutter/app/lib/core/bridge/MethodChannelCoreProxy.dart`

无数据库迁移、配置格式或公共 host API 变更。补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`，不要求其他增强版补丁。

## 验证

仅完成源码审阅。实际通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 读取原始代码，并以精确文本匹配生成最小补丁。

源码核对 watchStream 打开请求、yield* 和失败关闭的顺序；原有 dart:async 导入已提供 unawaited。

本次按提交者要求未运行构建、静态分析、自动化测试或设备测试；以下复现步骤依据调用链整理，尚未进行运行时复现。本 PR 对应单独提交并包含 DCO Signed-off-by。代码注释和以下复现步骤说明行为变化；未新增测试文件。

## 截图或日志

复现步骤：

1. 让 watch 打开请求在返回订阅前失败。
2. 消费相应 Dart 异步流。
3. 原错误处理等待未被监听的控制器 close，后续 yield* 无法执行。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
