## 变更说明

### 问题

重命名、删除、创建对话或向 agent 发消息等工具异常时，共用包装器统一返回“读取对话消息失败”，把用户和调用方引向错误操作。

### 原因

有参和无参两个通用包装器都复用了 read_messages 的固定错误文案，没有使用已经传入的实际执行函数。

### 修复

两处异常响应都显示当前 func.name，去掉内部 _impl 后缀，并保留原始异常信息；现有 success=false 和日志不变。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：所有运行 extended_chat 内置插件的平台；只修正工具失败时的用户可见错误文案。
- 模块：`plugins/packages/buildin/extended_chat.ts`。
- 兼容性：复用现有插件 API 和构建工具；无配置格式、数据库、host API 或 Flutter/Rust 边界变化。

## 验证

- 已读取 CONTRIBUTING.md 与 PR 模板；补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`。
- 在仅含原始基线文件的临时目录执行 `git apply --check --whitespace=error-all fix.patch`，独立应用检查通过。
- 源码检查：确认两处旧文案均位于共用异常包装器，func.name 已用于现有日志；对应函数采用稳定的 _impl 命名。此次不运行插件。
- 按本次工作范围未运行构建、插件同步、自动化测试或设备验证。下方步骤供审阅者执行，未作为实测结果报告。
- 未新增测试或改动公共操作文档：此改动只修正既有内部路径；复现和待验证行为记录在本 PR 中。

## 截图或日志

### 复现步骤（待验证）

1. 让 rename_chat_impl 或 delete_chat_impl 的底层调用抛出一个可识别错误。
2. 检查工具返回 message；原实现会误称读取对话消息失败。
3. 对无参工具包装器同样触发异常，确认错误中标出实际工具名并保留原异常文本。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
