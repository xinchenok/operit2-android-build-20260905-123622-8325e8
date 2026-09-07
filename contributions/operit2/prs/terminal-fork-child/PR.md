## 变更说明

### 问题

多线程应用创建 PTY 时，fork 后子进程仍分配 argv/envp 向量；若 fork 时其他线程持有分配器锁，子进程可能在 execve 前挂起，最终表现为启动超时。

### 复现步骤

1. 在其他应用线程频繁分配内存时重复创建 Android PTY。
2. 遇到启动超时后检查子进程是否停在 forkpty 与 execve 之间的 Vec 分配路径。

以上是根据现有控制流整理的复现路径，本次未在 Android 设备上执行。

### 原因

fork 只保留调用线程，其他线程持有的用户态锁可能处于不可恢复状态；当前子进程分支调用 collect 和 push，触发堆分配。

### 修复

在父进程调用 forkpty 前准备以空指针结尾的 argv/envp 指针数组；子进程仅沿用既有 chdir/execve/write/_exit 系统调用路径。

## 改动类型

- [x] Bug 修复
- [ ] 新功能
- [ ] 重构或性能改进
- [ ] 文档或构建工具
- [ ] 第三方依赖或许可证更新

## 影响范围

- 平台：Android。
- 模块：Android terminal host。
- 行为与兼容性：执行文件、参数和环境值保持一致，指针引用的 CString 在 fork/exec 期间仍由 command 持有；只调整分配发生的时机。
- host API 与 Flutter/Rust 边界：无接口、配置或数据库迁移。

## 验证

- 已读取上游 CONTRIBUTING.md 与 PR 模板，并逐处对照固定基线 `7fec1b2f17636c5b392c887fab28552219be807a` 的控制流和已有 API。
- 补丁生成时用精确、唯一匹配断言检查每处旧代码；每个 PR 独立从该上游基线生成完整文件，没有把其他 PR 当作前置分支。
- 已将受影响文件从固定基线导出到临时目录，执行 `git apply --check fix.patch`，独立应用检查通过。
- 本次按要求只做源码检查，未运行 cargo、Gradle、Flutter 构建、测试任务或 Android 设备复现；行为验证仍待维护者执行。
- 未添加公共接口或操作说明；本 PR 的复现路径记录了需要补做的设备验证，未新增无法在本次环境验证的设备测试。



## 提交者确认

- [x] 我已阅读 [CONTRIBUTING.md](https://github.com/AAswordman/Operit2/blob/main/CONTRIBUTING.md)。
- [x] 本提交包含 `Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。
- [x] 本次提交按账号所有者的明确授权执行，沿用项目许可证与现有版权声明。
- [x] 未引入第三方代码、资源或许可证变更。
- [x] 本补丁不含密钥、令牌、个人数据或本地构建产物。
- [x] 已在上文说明文档和测试处理方式。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
