## 变更说明

问题：一次群聊发送会等待规划模型及各个成员的回复完成，期间其他需要同一核心命令锁的请求无法进入，停止、工具回调或其他会话操作因此延迟。

原因：代理调用在持有核心对象锁时等待 sendUserMessage；群聊编排把远端规划和逐成员完成等待放在该调用的同步等待链中。单角色发送已有后台工作机制，但群聊编排仍占用外层调用。

修复：保留用户消息构造、写入和规划状态发布的原顺序；随后使用现有 host 异步任务调度器启动群聊后续流程，立即返回以结束调用者的持锁区间。后台协调器共享会话历史、处理状态和统计流，并复用已有非致命错误订阅；调度失败时发布错误状态。

范围边界：本 PR 只修复长时间占用核心命令锁的问题，不更改原有群聊停止语义。整轮取消的规划中断和剩余成员退出由独立的 `chat-group-cancellation` 修复。若共同合入，必须把取消 PR 在前半段捕获的 `cancellationVersion` 通过后台 `runGroupConversation` 的参数和调用传递，不能直接叠加后跳过集成检查。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：使用 Rust 群聊运行时的各平台。
- 模块：`MessageCoordinationDelegate` 的群聊发送任务边界。
- 兼容性：不新增 host API、依赖、配置或数据库迁移，不改变 Flutter/Rust 边界。

## 验证

已读取基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的 `CONTRIBUTING.md` 与 PR 模板。以下为实际完成的源码核对，不是运行时测试。

```text
python 通过 subprocess.check_output(['git', 'show', '7fec1b2f17636c5b392c887fab28552219be807a:<path>']) 读取原始源码。
核对 proxy/rust-codegen/src/build_rust_dispatch_codegen.rs：生成的派发在 holder 锁内 await 目标调用。
核对 ChatServiceCore.rs 和 MessageCoordinationDelegate.rs：发送等待链、用户消息先写入、后台任务只捕获拥有的数据。
核对 HostRuntimeAsyncTask、EnhancedAIService::clone、TokenStatisticsDelegate 及两类委托 clone_for_core：复用已有调度接口和共享状态。
补丁中的后台函数不引用 cancellationVersion；该标识不在原始基线中，已去除增强版的跨补丁依赖。
```

按本次任务要求未编译、未运行自动化测试或设备测试；下面的复现步骤供审阅者回归验证，尚未执行。此次仅修复内部控制流，未增加用户配置或文档用法；测试暂缺，不能据此声称已通过运行时验证。

## 截图或日志

复现步骤（源码推导，未在设备执行）：

1. 配置包含多个角色的群组，并让规划或首个角色的模型响应延迟。
2. 经核心代理发送群聊消息后，立即读取另一会话或执行其他需要同一核心对象的命令。
3. 确认用户消息已写入，而这些命令不必等待规划和全部角色回复才进入。
4. 模拟 host 异步任务调度失败，确认群聊进入 Error，且用户消息仍保留。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。

DCO 和权利确认保留为未勾选，供实际提交者确认；本目录仅准备补丁和 PR 正文，未创建提交。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
