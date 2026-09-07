## 变更说明

### 问题

插件同步的输入签名缓存仍在、plugins/.out 中的已编译 JavaScript 被清理时，同步跳过 tsc，随后复制阶段以 Compiled JavaScript not found 失败，不能自行恢复生成文件。

### 原因

_prebuild_plans 只比较源文件签名；.sync_state.json 与 .out 是独立产物，签名命中不说明后续 _sync 要读取的文件仍存在。

### 修复

对于现有 compile-ts 计划，仅在输入签名匹配且每个计划对应的 .out/<组>/<文件名>.js 都存在时跳过预构建；任一输出缺失便执行原有 tsc 命令。路径计算与 _sync 的实际读取路径一致，不改变缓存格式。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：所有调用 sync_plugin_packages.py 的构建平台；范围是顶层 compile-ts 计划的已知 JavaScript 输出，保持 script-packed ToolPkg 和子目录构建路径不变。
- 模块：`plugins/tools/sync_plugin_packages.py`。
- 兼容性：复用现有插件 API 和构建工具；无配置格式、数据库、host API 或 Flutter/Rust 边界变化。

## 验证

- 已读取 CONTRIBUTING.md 与 PR 模板；补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`。
- 在仅含原始基线文件的临时目录执行 `git apply --check --whitespace=error-all fix.patch`，独立应用检查通过。
- 源码检查：核对 _collect_sync_plan、_prebuild_plans、_sync 的路径与顺序，并用 ast.parse 检查生成 Python 的语法。现有五份源码快照中的同步脚本都与7fec一致，builder/plugin_payload.py 只有结果校验，没有这个缓存修复；本 PR 是这次源码核对后新准备的明确修复，不宣称是已落地的旧补丁。
- 按本次工作范围未运行构建、插件同步、自动化测试或设备验证。下方步骤供审阅者执行，未作为实测结果报告。
- 未新增测试或改动公共操作文档：此改动只修正既有内部路径；复现和待验证行为记录在本 PR 中。

## 截图或日志

### 复现步骤（待验证）

1. 正常同步一次内置插件，使 plugins/packages/buildin/.sync_state.json 记录当前输入签名。
2. 只删除 plugins/.out/buildin/extended_chat.js，保留源文件和状态文件。
3. 再次同步；原实现先输出 SKIP-PREBUILD，后报 Compiled JavaScript not found。
4. 修复后应先重新运行 tsc 再复制；输出完整且源未变时仍跳过预构建。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
