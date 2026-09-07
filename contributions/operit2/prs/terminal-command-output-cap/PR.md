## 变更说明

### 问题

持续大量输出的命令虽然共享队列最多保留 1 MiB，但执行等待循环会把所有批次累积到另一个无上限 Vec，直到命令完成或超时，可能耗尽应用内存。

### 复现步骤

1. 在终端执行持续输出命令并设置较长 timeout。
2. 观察 drainAndroidPtyCommandOutput 持续将共享队列内容追加到 collected。
3. 原实现 collected 长度随总输出增长，超过 PTY_OUTPUT_LIMIT 后仍不裁剪。

以上是根据现有控制流整理的复现路径，本次未在 Android 设备上执行。

### 原因

上限只施加在 reader 共享队列上；消费者反复排空该队列，让用于完成标记解析和命令结果的第二缓冲无限增长。

### 修复

批量 drain 后将 collected 长度限制在已有 PTY_OUTPUT_LIMIT 内，丢弃最旧内容并在开头加入明确的截断提示，保留最新输出及靠近结尾的完成标记。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：Android terminal host。
- 行为与兼容性：超长命令结果现在只保留约 1 MiB 的最新内容，并显示 `[Earlier terminal output was truncated]`；普通输出和现有 prompt 解析接口不变。
- host API 与 Flutter/Rust 边界：无接口、配置或数据库迁移。

## 验证

- 已读取上游 CONTRIBUTING.md 与 PR 模板，并逐处对照固定基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的控制流和已有 API。
- 补丁生成时用精确、唯一匹配断言检查每处旧代码；每个 PR 独立从该上游基线生成完整文件，没有把其他 PR 当作前置分支。
- 已将受影响文件从固定基线导出到临时目录，执行 `git apply --check fix.patch`，独立应用检查通过。
- 本次按要求只做源码检查，未运行 cargo、Gradle、Flutter 构建、测试任务或 Android 设备复现；行为验证仍待维护者执行。
- 未添加公共接口或操作说明；本 PR 的复现路径记录了需要补做的设备验证，未新增无法在本次环境验证的设备测试。



## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1（由实际提交者提交时确认）。
- [ ] 我拥有本次提交内容的版权或必要授权（由实际提交者确认）。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。
