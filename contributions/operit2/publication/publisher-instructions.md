> 历史准备说明：发布已于任务34119116134成功完成，89个PR已创建。当前结果见 [PR清单](README.md)；无需再次配置授权或触发提交。

# Operit2 上游 PR 提交入口

89 个修复分支已经写入 `xinchenok/Operit2` 并核对提交号；每个分支只有一个问题的修复。此入口将按固定清单向 `AAswordman/Operit2:main` 创建89个 Draft PR，不合并、不请求审阅者、不启动 APK 构建。

当前已创建上游 PR：**0**。ChatGPT 的 GitHub 连接创建 PR 时返回 `403 Resource not accessible by integration`。此工作流需要你自行配置新的有效授权，当前连接的权限不会因此改变。

## 配置一次授权

1. 打开 [创建经典令牌](https://github.com/settings/tokens/new?scopes=public_repo&description=Operit2-upstream-PRs)，选择短有效期，权限仅勾选 `public_repo`。该范围允许操作你有权限访问的公开仓库，并非仅限这89个PR；不需要完整 `repo` 或 `workflow` 范围。
2. 打开 [新建仓库 Actions Secret](https://github.com/xinchenok/Operit2/settings/secrets/actions/new)，Name 填 `UPSTREAM_PR_TOKEN`，Secret 粘贴刚生成的令牌。仅保存到 GitHub，不要发送到聊天里。
3. 打开 [Actions](https://github.com/xinchenok/Operit2/actions)。如果页面提示 fork 的工作流未启用，点击启用。完成后告知助手“已配置”，即可继续触发本入口；不需要在电脑上运行脚本。

GitHub 对非仓库成员向公开仓库贡献的场景仍列有 fine-grained token 限制，所以这里使用 classic token。依据：[令牌限制](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens)、[公开仓库范围](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps)。

## 已准备的执行方式

完整标题、说明和精确分支提交号在 [manifest.json](.github/upstream-prs/manifest.json)，发布脚本在 [publish.py](.github/upstream-prs/publish.py)。工作流仅由专用分支 `contributions/submit-upstream-prs` 中 `.github/upstream-prs/request.txt` 的变更触发；本次准备未创建该文件，没有启动发布任务。

触发后会逐个记录 PR 链接；已有的 PR（包括关闭状态）直接跳过。网络断线导致请求结果不明时先查是否创建成功；真实权限错误会保留，不盲目重试。限流按 GitHub 返回的等待时间处理。Actions Summary 会列出全部结果，重跑不会重复开同一分支的 PR。

修复分支已通过独立补丁应用检查，未独立编译或手机实测，所以均作为 Draft PR 提交。另有一份角色供应商导入衔接补丁需要两项前置修复落地，不混入当前89个独立 PR；[完整修复材料与依赖说明](https://github.com/xinchenok/operit2-android-cx/tree/contributions/operit2-bugfix-prs-20260907/contributions/operit2)。
