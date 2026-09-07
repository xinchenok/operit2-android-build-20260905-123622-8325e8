## 变更说明

### 问题

命令超时后，即使 Ctrl-C 后 3 秒内仍未出现 shell 提示符，原实现也把会话标记为空闲并继续复用。尚未退出的前台程序可能把下一条命令当作标准输入。

### 原因

`cancelTimedOutAndroidPtyCommand` 用 `Ok(None)` 表示没有等到恢复提示符，调用方却只处理 `Some(workingDir)`，将没有恢复的状态当作可继续使用。取消错误也会提前退出而保留该会话。

### 修复

明确区分收到提示符与恢复失败：仅 `Ok(Some(workingDir))` 保留会话；没有提示符或取消失败时移除 PTY 及两类名称映射。调用方仍收到本次命令已收集的输出与 `timedOut = true`。提前回收路径同时需要一个最小 Drop 保护：已记录退出码的进程不再发送信号，避免对已回收 PID 重复 kill。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：`hosts/android/src/terminal.rs`。
- 行为：无法在既有取消窗口内恢复的超时会话不再复用；旧 session ID 失效，其 shell 内存状态随会话结束。可见名称与 hidden executor 的下一次使用可创建新会话，不延长现有取消窗口。
- 兼容性：复用现有 Android host API；不修改 Rust runtime 与 Flutter 的接口边界，无配置、数据库或数据迁移。

## 验证

- 基线：`AAswordman/Operit2` 的 `7fec1b2f17636c5b392c887fab28552219be807a`，每个补丁独立基于该提交生成。
- 已执行：`git apply --check --whitespace=error-all <本 PR 的 fix.patch>`，在仅含原始基线文件的临时目录检查，结果通过。
- 已完成源码审阅：确认三种取消结果均被显式处理；退役同时删除 PTY 和两类映射，成功恢复仍更新工作目录，超时返回字段仍采用原始收集结果。
- 按本次工作范围，仅做源码检查；未运行编译、单元测试、Flutter/Rust 构建或 Android 设备验证。下列步骤是待执行的设备复现步骤，并非已验证的运行结果。
- 未新增测试或修改用户文档：此 PR 仅调整 host 内部实现；设备运行结果需在具备 Android PTY 环境后补充。

## 截图或日志

### 复现步骤（待设备验证）

1. 在 bash session 中运行忽略 SIGINT 且持续超过超时与取消等待总时长的程序，例如一个忽略 SIGINT、休眠 30 秒后读取标准输入的 Python 程序。
2. 把 `timeoutMs` 设为较短值；等待该请求超时返回后，立刻使用同名会话或同一 hidden executor key 发送另一条命令。
3. 原 PTY 未回到提示符时，新命令可能进入旧程序的输入队列。修复后旧 session ID 被移除，同名/hidden 请求创建新 PTY；能正常响应 Ctrl-C 的会话则继续保留。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。（当前为待提交补丁；正式提交时由实际贡献者签署。）
- [ ] 我拥有本次提交内容的版权或必要授权。（由实际提交者确认。）
- [x] 我已保留第三方代码、资源及其许可证和版权声明。（未改动第三方文件。）
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
