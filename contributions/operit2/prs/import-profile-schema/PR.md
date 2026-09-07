## 变更说明

复现步骤：导入使用 memory_space_list / active_memory_space_id 的已发布 Operit1 快照，或元数据记录缺失但实际数据库、角色绑定仍存在的快照。

原因：导入器仅识别旧 profile_* 字段，把必填元数据缺失当作整体失败，并忽略未被角色引用的空间。

修复：按已发布迁移优先级读取旧 profile 或新 memory_space schema，汇总列表、实际数据库和绑定中的身份；缺失记录仅补 ID/名称标签，空空间建立空记忆库。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

跨平台 Rust 导入流程；沿用 RuntimeStorageHost / RuntimeSqliteHost，不改变 Flutter bridge 或 host API。

## 验证

已阅读基线提交 `7fec1b2f17636c5b392c887fab28552219be807a` 的相关调用链，并逐项核对补丁涉及的现有类型和方法。补丁为独立基线变更。

依照本次任务要求，未运行编译、单元测试或设备测试；不声称实际复现或运行通过。以下复现为源码推导的手动验证步骤，提交前仍需按 CONTRIBUTING.md / BUILDING.md 执行相关平台检查。

只解决空间身份与元数据兼容性；新 schema 的 user.md 内容恢复另见 import-user-documents。合成记录不虚构用户正文。

## 提交者确认

- [x] 已阅读 CONTRIBUTING.md 和 PR 模板。
- [ ] 每个最终提交包含 Signed-off-by 并符合 DCO 1.1（此阶段仅准备补丁，未创建提交）。
- [ ] 最终提交者确认版权或必要授权。
- [x] 补丁未加入第三方代码、资源、密钥、令牌或个人数据。
- [x] 已说明本次未运行测试的原因。
