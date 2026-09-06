# Operit2 Android：GitHub 一键构建最新代码

个人 ARM64 构建，非官方发行版。编译全部在 GitHub Actions 运行，不需要在自己的电脑上安装或执行启动器。

## 首次设置：保存一次固定签名

打开 [新增仓库 Secret](https://github.com/xinchenok/operit2-android-build-20260905-123622-8325e8/settings/secrets/actions/new)。

- Name：`OPERIT2_UPDATE_SIGNING`
- Secret：私下交付的 `OPERIT2_UPDATE_SIGNING.txt` 的全部内容，即一个完整 JSON 对象。

点击 **Add secret**。签名文件包含私钥及口令，不要提交到公开仓库、Issue 或日志；请私下保存备份。已有固定签名时复用原 Secret，不要换新密钥。无需运行 PowerShell、GitHub CLI 或本地脚本。

## 以后每次构建

打开 [Build latest Android](https://github.com/xinchenok/operit2-android-build-20260905-123622-8325e8/actions/workflows/update-android.yml)，点击 **Run workflow**，保留 `main`，再确认 **Run workflow**。没有需要填写的自定义参数。

每轮开始时查询 `AAswordman/Operit2` 默认分支的最新提交，运行库和 APK 使用同一次解析到的源码。始终复用固定签名，递增 Android versionCode。上游和构建脚本均未改变、且已有对应成功 Release 时，跳过重复编译。

成功后打开 [最新 APK 下载页](https://github.com/xinchenok/operit2-android-build-20260905-123622-8325e8/releases/latest)，下载 `operit2-android-arm64-personal-test.apk`；运行摘要也有下载入口。

获取新源码请用 **Run workflow** 新建运行。旧运行的 **Re-run failed jobs** 用于恢复那一轮已选定的源码，不是重新追踪最新提交。

## 签名与旧版

后续使用同一把个人签名私钥，不是每轮随机生成，也不靠会过期的缓存保存。它不是作者的官方签名。

签名不同的旧 APK 不能直接覆盖安装。请先备份应用数据，在新 APK 成功生成后再处理首次迁移，不要提前卸载旧应用。迁移到固定签名版本后，后续构建保持包名、签名并递增版本号。

旧的固定提交、一次性签名工作流已经退役，只保留跳转提示。不要再使用 `Start-Build.cmd` 或 `Update-Operit2.cmd`。

## 验证状态

`Check cloud build and fixed signing (no Android build)` 只检查脚本和签名导入，不编译 APK。检查通过不代表全量 Android 构建成功；以 **Build latest Android** 的运行结果和实际 Release 产物为准。上游最新开发代码仍可能存在编译或运行问题。
