## 变更说明

### 问题

Android system shell 的命令执行失败后，OperitPrompt 仍可能报告退出码 0，调用方无法据此判断失败。

### 原因

原循环在循环体开头执行 `__operit_status=$?`，但此时 `$?` 来自刚执行过的 `while true` 条件，而不是上一轮 `eval` 的返回值。

### 修复

初始化首次提示符的状态为 0，并在每次 `eval` 之后立即保存退出码；下一轮只读取保存值生成提示符。初始目录切换失败则退出，保留原来无法进入命令循环的行为。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：`hosts/android/src/terminal.rs`。
- 行为：仅 Android system shell 的退出码报告得到纠正；bash 的 PROMPT_COMMAND 不变，提示符协议及 host API 结构不变。依赖错误退出码的上层流程可获得真实结果。
- 兼容性：复用现有 Android host API；不修改 Rust runtime 与 Flutter 的接口边界，无配置、数据库或数据迁移。

## 验证

- 基线：`AAswordman/Operit2` 的 `7fec1b2f17636c5b392c887fab28552219be807a`，每个补丁独立基于该提交生成。
- 已执行：`git apply --check --whitespace=error-all <本 PR 的 fix.patch>`，在仅含原始基线文件的临时目录检查，结果通过。
- 已完成源码审阅：按 shell 控制流检查 `$?` 的采集位置，确认位于 `eval` 与下一次 `while true` 之间；核对初始化、目录切换失败与输入 EOF 的退出路径。
- 按本次工作范围，仅做源码检查；未运行编译、单元测试、Flutter/Rust 构建或 Android 设备验证。下列步骤是待执行的设备复现步骤，并非已验证的运行结果。
- 未新增测试或修改用户文档：此 PR 仅调整 host 内部实现；设备运行结果需在具备 Android PTY 环境后补充。

## 截图或日志

### 复现步骤（待设备验证）

1. 用 `android-system` / `shell` 创建显式 PTY session。
2. 依次执行 `false`、`true` 与 `sh -c 'exit 7'`。
3. 检查 `TerminalCommandOutput.exitCode`；期望依次为 1、0、7，原循环会在采集状态之前用 `while true` 覆盖失败状态。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 我已保留第三方代码、资源及其许可证和版权声明。（未改动第三方文件。）
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
