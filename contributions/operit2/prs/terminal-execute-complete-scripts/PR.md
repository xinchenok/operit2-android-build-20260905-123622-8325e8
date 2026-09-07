## 变更说明

### 问题

多行 command 被逐行粘贴进交互式 PTY 后，前面的语句即可产生完成提示符。后续语句运行较久或等待输入时，host 可能把中间提示符当作整段命令完成，提前返回并遗漏后续输出。

### 原因

原实现把 `normalizedCommand` 与回车直接写入 PTY，交互式 shell 可在换行分隔的语句之间输出完成标记；收集器无法区分这些标记是否属于整个 command。

### 修复

把 command 写入带 UUID 的临时脚本，仅向 PTY 提交一条 `. '路径'`。bash 使用 rootfs 中的 `/tmp` 路径，system shell 使用现有 runtime tmp 路径；路径单引号转义后引用。通过 source 保留原 shell 中的工作目录、变量等状态，输出去回显逻辑匹配实际发送的包装命令；RAII 在函数返回时删除临时文件。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：`hosts/android/src/terminal.rs`。
- 行为：多行命令按单个脚本执行并返回完整完成结果，单行命令同样经过脚本。脚本保存在现有 Android runtime/rootfs 临时目录，不引入新 API 或依赖。使用脚本级 `return` 等语法时遵循 shell 的 source 语义。
- 兼容性：复用现有 Android host API；不修改 Rust runtime 与 Flutter 的接口边界，无配置、数据库或数据迁移。

## 验证

- 基线：`AAswordman/Operit2` 的 `7fec1b2f17636c5b392c887fab28552219be807a`，每个补丁独立基于该提交生成。
- 已执行：`git apply --check --whitespace=error-all <本 PR 的 fix.patch>`，在仅含原始基线文件的临时目录检查，结果通过。
- 已完成源码审阅：核对脚本的 host 路径与 shell 可见路径、现有 runtime 环境变量、单引号引用、UUID 命名和 Drop 清理；核对返回 command 仍为用户输入，去回显参数则为实际发送的 source 命令。
- 按本次工作范围，仅做源码检查；未运行编译、单元测试、Flutter/Rust 构建或 Android 设备验证。下列步骤是待执行的设备复现步骤，并非已验证的运行结果。
- 未新增测试或修改用户文档：此 PR 仅调整 host 内部实现；设备运行结果需在具备 Android PTY 环境后补充。

## 截图或日志

### 复现步骤（待设备验证）

1. 在 Android bash session 执行包含实际换行的 command：
   ```sh
   printf 'first\n'
   sleep 1
   printf 'last\n'
   ```
2. 检查返回是否发生在 `sleep` 结束之前，以及输出是否缺少 `last`。
3. 再执行多行 `cd /tmp` 和 `export OPERIT_SCRIPT_CHECK=ready`，随后用另一条命令检查目录与变量仍在同一 shell 中保留。
4. 对 android-system/shell 的显式 session 重复多行命令，并检查结束后临时脚本已清理。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [ ] 我的每个提交都包含 `Signed-off-by`，并符合 DCO 1.1。（当前为待提交补丁；正式提交时由实际贡献者签署。）
- [ ] 我拥有本次提交内容的版权或必要授权。（由实际提交者确认。）
- [x] 我已保留第三方代码、资源及其许可证和版权声明。（未改动第三方文件。）
- [x] 我没有提交密钥、令牌、个人数据或本地构建产物。
- [x] 我已更新受影响的文档和测试，或在上文说明了原因。
