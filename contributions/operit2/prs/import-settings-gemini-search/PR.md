## 变更说明

Operit1 模型配置中的 `enableGoogleSearch` 在导入时未反序列化，生成的模型也未设置内置搜索工具。即使源配置开启了 Google 搜索，导入后的 Gemini 请求仍不会启用搜索。原版已具备 `GeminiGoogleSearch` 请求格式、内置工具配置和请求序列化能力，缺失的是导入映射。

读取 `enableGoogleSearch`（旧快照缺省为 false），对 `GOOGLE` 和 `GEMINI_GENERIC` 模型设置原版 `builtinToolsOverride`，保留开启和关闭状态；沿用现有搜索与外部工具互斥规则。移除导入结果中已支持的 `enableGoogleSearch` 跳过项。该映射不要求模型已列入内置目录。

本次从增强补丁中只抽取原版现有能力的修复。不加入 `contextLength`、`summaryCustomRules` 或增强版多供应商导入流程；保留 `contextLength`、`summaryCustomRules`、`enableClaude1hPromptCache` 的原版跳过提示。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：所有调用 Rust 快照导入流程的平台。
- 模块：Operit1 模型配置导入；Flutter/Rust 边界和供应商请求实现不变。

配置格式、数据库结构和 host API 均不变；不需要额外迁移。

## 验证

已通过 `git show 7fec1b2f17636c5b392c887fab28552219be807a:<path>` 阅读以下相关源码，并核对拟议差异：`Operit1SnapshotImportManager.rs` 的导入、反序列化和模型构造；`ModelConfigData.rs` 的原版工具类型；`ModelConfigManager.rs` 的内置工具解析；`GeminiProvider.rs` 的启用检测和请求构造。

本次任务明确要求仅做源码审查及补丁准备，因此未运行编译、单元测试、Flutter 分析或设备测试。下列复现步骤由源码推导，尚未实际执行；不将其表述为测试通过。最终提交前仍需依照 CONTRIBUTING.md / BUILDING.md 完成相关检查。改动直接修复现有输入、保存或显示行为，未新增功能文档和测试文件。

## 截图或日志

建议手动复现步骤（未执行）：

1. 导入含 `GOOGLE` 或 `GEMINI_GENERIC` 模型且 `enableGoogleSearch=true` 的 Operit1 快照；检查导入模型的内置 Google 搜索已开启。
2. 使用该模型发送请求；预期现有 Gemini 请求逻辑生成 `googleSearch` 工具，且遵守原有互斥规则。
3. 将该字段设为 false，或使用未含此字段的旧快照；预期搜索保持关闭。
4. 导入其他供应商配置；预期不会生成 Gemini 搜索工具，未支持字段仍保留在导入提示中。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1（本阶段仅准备补丁，尚未创建提交）。
- [ ] 我拥有本次提交内容的版权或必要授权（由最终提交者确认）。
- [x] 我已保留第三方代码、资源及其许可证和版权声明；本补丁没有新增第三方内容。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
