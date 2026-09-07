## 变更说明

### 问题

直接 PTY 写入与终端 input API 在持有全局会话状态锁时写入；一个停止读取输入的子进程会阻塞关闭会话、查询屏幕及其他会话的操作。

### 复现步骤

1. 创建两个 PTY，会话 A 运行持续输出且暂不读取 stdin 的进程。
2. 通过 writePtySession 或 inputInSession 向 A 发送足以产生背压的大量输入。
3. 同时查询会话 B 或关闭 A；原实现会等待 A 的写入释放全局 state mutex。

以上是根据现有控制流整理的复现路径，本次未在 Android 设备上执行。

### 原因

这两条输入路径把查找 session 与可能阻塞的 writeAndroidPtyBytes 放在同一 state 锁范围内。释放全局锁后也需要保证并发 close 不会让 writer 使用复用 fd。

### 修复

在短暂 state 锁内克隆 Arc writer 后再写入；输入辅助函数仅接收 writer。包含独立应用所需的写端所有权和 nonblocking 重试：单次 syscall 保持 fd 锁，每次尝试后释放，短写继续，5 秒无进展报错。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：Android terminal host。
- 行为与兼容性：其他会话和关闭操作可在输入背压期间继续；保持 input/control 编码与 acceptedChars 规则，错误会明确显示输入进度。
- host API 与 Flutter/Rust 边界：无接口、配置或数据库迁移。

## 验证

- 已读取上游 CONTRIBUTING.md 与 PR 模板，并逐处对照固定基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的控制流和已有 API。
- 补丁生成时用精确、唯一匹配断言检查每处旧代码；每个 PR 独立从该上游基线生成完整文件，没有把其他 PR 当作前置分支。
- 已将受影响文件从固定基线导出到临时目录，执行 `git apply --check fix.patch`，独立应用检查通过。
- 本次按要求只做源码检查，未运行 cargo、Gradle、Flutter 构建、测试任务或 Android 设备复现；行为验证仍待维护者执行。
- 未添加公共接口或操作说明；本 PR 的复现路径记录了需要补做的设备验证，未新增无法在本次环境验证的设备测试。

写端所有权与 nonblocking 的必要部分也出现在 `terminal-descriptor-lifetime`；本 PR 重复这些配套改动以独立、安全地缩小 state 锁范围。旧 reader 的 fd 生命周期由该单独 PR 修复。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1（由实际提交者提交时确认）。
- [ ] 我拥有本次提交内容的版权或必要授权（由实际提交者确认）。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。
