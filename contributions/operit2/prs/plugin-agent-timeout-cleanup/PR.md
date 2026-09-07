## 变更说明

### 问题

agent_send 已获得回复或因发送错误结束后，本次调用创建的 timeout 定时器仍保留到原始截止时间；多次快速结束的调用会累积尚未触发的定时器与 Promise 回调。

### 原因

Promise.race 只决定返回哪个结果，不会取消另一个 Promise 的 setTimeout；原实现没有保留定时器句柄，也没有在成功或拒绝后清理。

### 修复

保存现有 setTimeout 返回的句柄，并在 race 的 finally 中清除；成功、发送拒绝和超时都走同一资源释放路径。保留现有发送、超时结果及错误传播。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：所有运行 extended_chat 内置插件的平台；仅改变 agent_send 的计时器资源生命周期。
- 模块：`plugins/packages/buildin/extended_chat.ts`。
- 兼容性：复用现有插件 API 和构建工具；无配置格式、数据库、host API 或 Flutter/Rust 边界变化。

## 验证

- 已读取 CONTRIBUTING.md 与 PR 模板；补丁独立基于 `7fec1b2f17636c5b392c887fab28552219be807a`。
- 在仅含原始基线文件的临时目录执行 `git apply --check --whitespace=error-all fix.patch`，独立应用检查通过。
- 源码检查：对照 builder/plugin_payload.py 中现有 timer_cleanup 修复；确认句柄类型沿用现有 setTimeout 声明，finally 保留 Promise.race 的完成值和拒绝原因。
- 按本次工作范围未运行构建、插件同步、自动化测试或设备验证。下方步骤供审阅者执行，未作为实测结果报告。
- 未新增测试或改动公共操作文档：此改动只修正既有内部路径；复现和待验证行为记录在本 PR 中。

## 截图或日志

### 复现步骤（待验证）

1. 调用 agent_send，设置较长 timeout，让发送在短时间内成功或失败。
2. 在可观察计时器的插件调试环境检查该调用遗留的 timeout；原实现会一直保留到截止时间。
3. 连续重复调用，确认修复后每次结束即清理该定时器；另验证真正超时仍返回原有 timeout 结果。

## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
