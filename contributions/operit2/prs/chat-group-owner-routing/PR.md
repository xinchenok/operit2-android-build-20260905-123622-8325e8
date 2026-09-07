## 变更说明

问题：显式指定会话的群聊发送会跳过群聊编排；没有显式覆盖时，已绑定群组的会话也可能采用全局当前选中的另一群组。

原因：`shouldRunGroupOrchestration` 对非空 `chatIdOverride` 直接返回 false；编排入口只读取当前会话；`resolveTargetGroupForChat` 虽读取会话绑定，却丢弃结果并使用全局活动群组。

修复：选择发送目标时优先使用非空 `chatIdOverride`，并用相同目标解析是否需要群聊编排。解析群组时优先尊重会话的群组绑定；已绑定单角色的会话不借用其他界面选中的群组，未绑定会话保留活动群组回退。



## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：使用 Rust 群聊运行时的各平台。
- 模块：`MessageCoordinationDelegate` 的发送路由和群组解析。
- 兼容性：不新增 host API、依赖、配置或数据库迁移，不改变 Flutter/Rust 边界。

## 验证

已读取基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的 `CONTRIBUTING.md` 与 PR 模板。以下为实际完成的源码核对，不是运行时测试。

```text
python 通过 subprocess.check_output(['git', 'show', '7fec1b2f17636c5b392c887fab28552219be807a:<path>']) 读取原始源码。
核对 MessageCoordinationDelegate.rs：目标会话选择、编排条件、群组绑定读取与 updateChatCharacterBinding 的调用链。
核对 ChatServiceCore.rs：chatIdOverride 传入协调器；没有增加上游不存在的服务。
```

按本次任务要求未编译、未运行自动化测试或设备测试；下面的复现步骤供审阅者回归验证，尚未执行。此次仅修复内部控制流，未增加用户配置或文档用法；测试暂缺，不能据此声称已通过运行时验证。

## 截图或日志

复现步骤（源码推导，未在设备执行）：

1. 创建分别绑定群组 A 和 B 的两个会话，切换到 B。
2. 通过带 A 会话 ID 的发送入口向 A 发送一条消息，确认仍进入 A 群组的多角色编排。
3. 保持全局活动群组为 B，再向已绑定 A 的会话发送，确认不会改绑 B。
4. 向已绑定单角色的会话发送，确认不会被当前活动群组接管。

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
