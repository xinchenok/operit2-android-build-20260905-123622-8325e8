## 问题与复现

原生 Flutter 在 Core node 建立之前执行 application-on-create ToolPkg 钩子。依赖已注册节点/host 的启动钩子会在宿主准备完成前运行，首次启动和普通运行时调用表现不一致。

## 原因与修复过程

`onCreate` 同时初始化服务和派发插件启动事件，Flutter 当时尚未完成后续 node 创建。将初始化与派发拆为内部步骤：原 `onCreate` 保留其他宿主行为，原生 Flutter 选择延后派发，在 node 创建成功后再派发同一个原始事件。保持 Wasm 原路径。

## 影响与验证

仅调整原生 Flutter 的启动事件时序；不导入增强版工具、额外命令执行器或工作流。已核对事件派发位置、原始 payload 和宿主分支；未独立编译或手机实测。可用读取当前 node 信息的 application-on-create 钩子对比前后行为。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
