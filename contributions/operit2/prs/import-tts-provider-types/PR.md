## 变更说明

复现步骤：导入 OpenAI、MiniMax、MiMo、SiliconFlow 或豆包 TTS 配置。旧流程把所有非系统服务都标成 HTTP_TTS，遗漏已有目录中的供应商默认请求和响应处理。

原因：旧转换没有按源服务类型选择现有 TtsCatalog 项，统一按通用 HTTP 模板解释专用供应商。

修复：把已有云端服务映射到对应目录类型，恢复其默认模型、请求格式、响应管线和必要请求头；用户自定义 HTTP_TTS 继续保留原始配置。

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

已直接核对 pristine TtsCatalog 包含上述供应商。OPENAI_WS_TTS、VITS 和语音多配置策略不在此补丁中；没有加入新驱动。

## 提交者确认

- [x] 已阅读 CONTRIBUTING.md 和 PR 模板。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 补丁未加入第三方代码、资源、密钥、令牌或个人数据。
- [x] 已说明本次未运行测试的原因。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
