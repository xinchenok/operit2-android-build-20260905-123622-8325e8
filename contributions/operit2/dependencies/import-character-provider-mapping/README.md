# 角色供应商导入组合适配

该补丁必须在 `model-fixed-character-binding` 和 `import-all-model-configs` 都落地后应用，不是可直接对 7fec 提交的独立 PR。

问题：角色字段加入 provider ID 后，完整导入又为供应商生成新 ID；若不把源 ID 映射回角色，已导入角色仍无法选择原供应商，尤其是多家供应商使用同名模型时。

修复：完整导入收集源→目标供应商 ID，角色固定配置按这张表恢复 provider/model 对；缺失配置保留角色并报告重新选择，不猜测其他供应商。单模型导入不请求身份映射。

这份适配包含角色失效模型的容错，与 `import-stale-character-models` 重叠；若后者已合并，应保留同一逻辑一次。没有修改共享源码，也未运行编译或测试。
