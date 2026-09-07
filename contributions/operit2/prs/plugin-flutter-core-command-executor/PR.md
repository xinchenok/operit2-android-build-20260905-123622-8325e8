## 问题与复现

Flutter 原生宿主中的 JavaScript 插件调用既有 Core command 能力时，host 的 `coreCommandExecutor` 未绑定。调用该能力的工具会返回执行器不可用，而 CLI 有自己的命令入口。

## 原因与修复过程

Flutter 创建 local Core 后只取出聊天和存储能力，没有把本应用的命令入口交给工具上下文。为已初始化的应用提供共享服务上下文，将 `run_core_command` 绑定到现有 host 回调，并同步给工具 handler。命令在现有 native task scheduler 上运行，避免在插件回调中再次持有应用 dispatch 锁；服务是原应用的共享句柄，不创建另一份聊天状态。

## 影响与验证

仅原生 Flutter 宿主启用此绑定；Wasm 分支保留原逻辑。不导入旧包、不加入 Java/Android 桥新功能。本 PR 处理正常运行时命令入口，启动钩子时序单独列出。已核对能力类型、共享对象和 cfg 边界；未独立编译或执行插件实测。
