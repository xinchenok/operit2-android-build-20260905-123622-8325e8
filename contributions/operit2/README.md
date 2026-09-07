# Operit2 单问题修复提交准备清单

已准备 **89 份独立 PR 材料**，每份包含一个问题的补丁、修复说明和元数据；另有一份需要两项前置修复的角色供应商导入衔接补丁。

**89个独立修复PR已全部提交成功，编号#8–#96，当前均为open Draft。** 每个预定分支恰好对应一个PR，提交号与准备清单一致，目标均为 `AAswordman/Operit2:main`。

- [全部89个PR的实际链接](publication/README.md)
- [完整发布与核对结果](publication/results.json)
- [成功的云端提交任务](https://github.com/xinchenok/Operit2/actions/runs/34119116134)

此前连接创建PR的403由用户另行配置授权后，通过专用GitHub Actions任务完成提交。原连接权限没有改变。本次没有运行APK构建或独立编译/手机实测，没有合并PR。另有一份角色供应商导入衔接补丁仍需前置修改落地后处理。

基线：[7fec1b2](https://github.com/AAswordman/Operit2/commit/7fec1b2f17636c5b392c887fab28552219be807a)。增强修复来源：`18017fc00201df9ed943b1d7245f4b21717556d8`。逐条核对了 **538 个替换片段**、28 个 overlay 文件、旧包源码适配，以及早期工具包/构建修复。片段数量不是 bug 数量。

## 验证与拆分边界

全部独立补丁通过隔离 Git index 的应用检查；每份说明列明源码判断依据、复现步骤、修改流程和影响范围。没有重新开启 APK 构建，也没有独立编译或手机实测，不能称为全部功能验证通过。

各 PR 独立基于同一上游提交，必要公共前提可能出现重叠，合入时需要协调相同代码；它们不是可按编号无冲突连续应用的补丁系列。89条修复 commit 已按上游 DCO 包含已核对的身份：`Signed-off-by: xinchenok <95321008+xinchenok@users.noreply.github.com>`。

新增旧版引擎、Java/Android 兼容宿主、第三方免责声明和个人构建发布配置已单独列为扩展；不把上游不存在的新增功能说成上游既有 bug。之前提到的插件同步缓存修复实际尚未落地，本次已据基线补成 `plugin-sync-missing-outputs`，覆盖记录明确注明这是新准备的修复。

- [机器可读完整清单与逐片段覆盖](manifest.json)
- [独立补丁应用检查](patch-application-check.json)
- [角色供应商导入衔接补丁及前置条件](dependencies/import-character-provider-mapping/README.md)
- [衔接补丁应用检查](dependency-application-check.json)
- [不属于上游源代码的构建改动](coverage/build-only-changes.json)

## 独立 PR 清单

| 编号 | 拟提交 PR / 修复说明 | 补丁 |
|---:|---|---|
| 1 | [fix(chat): separate attachment cleanup from send errors](prs/chat-accepted-attachment-cleanup/PR.md) | [fix.patch](prs/chat-accepted-attachment-cleanup/fix.patch) |
| 2 | [fix(chat): cancel stale message locator jumps](prs/chat-cancel-pending-locator/PR.md) | [fix.patch](prs/chat-cancel-pending-locator/fix.patch) |
| 3 | [fix(chat): guard detached scroll controllers](prs/chat-detached-scroll-navigator/PR.md) | [fix.patch](prs/chat-detached-scroll-navigator/fix.patch) |
| 4 | [fix(chat): stop group planning and pending member turns on cancellation](prs/chat-group-cancellation/PR.md) | [fix.patch](prs/chat-group-cancellation/fix.patch) |
| 5 | [fix(chat): release the core command lock during group responses](prs/chat-group-command-lock/PR.md) | [fix.patch](prs/chat-group-command-lock/fix.patch) |
| 6 | [fix(chat): resolve group orchestration from the target conversation](prs/chat-group-owner-routing/PR.md) | [fix.patch](prs/chat-group-owner-routing/fix.patch) |
| 7 | [fix(chat): preserve drafts changed during submit hooks](prs/chat-hook-draft-ownership/PR.md) | [fix.patch](prs/chat-hook-draft-ownership/fix.patch) |
| 8 | [fix(chat): initialize the first conversation](prs/chat-initial-conversation/PR.md) | [fix.patch](prs/chat-initial-conversation/fix.patch) |
| 9 | [fix(markdown): assign unique paragraph break keys](prs/chat-markdown-paragraph-keys/PR.md) | [fix.patch](prs/chat-markdown-paragraph-keys/fix.patch) |
| 10 | [fix(chat): ignore nested scroll notifications](prs/chat-nested-scroll-notifications/PR.md) | [fix.patch](prs/chat-nested-scroll-notifications/fix.patch) |
| 11 | [fix(chat): retain partial answers after stream failures](prs/chat-preserve-partial-answer/PR.md) | [fix.patch](prs/chat-preserve-partial-answer/fix.patch) |
| 12 | [fix(chat): preserve edits while loading queued input](prs/chat-queue-edit-draft/PR.md) | [fix.patch](prs/chat-queue-edit-draft/fix.patch) |
| 13 | [fix(chat): restore queued input rejected by hooks](prs/chat-queue-hook-restoration/PR.md) | [fix.patch](prs/chat-queue-hook-restoration/fix.patch) |
| 14 | [fix(chat): keep draft attachments out of queued sends](prs/chat-queued-text-attachments/PR.md) | [fix.patch](prs/chat-queued-text-attachments/fix.patch) |
| 15 | [fix(chat): retain the submitted reply target](prs/chat-reply-ownership/PR.md) | [fix.patch](prs/chat-reply-ownership/fix.patch) |
| 16 | [fix(chat): respect cancelled bottom following](prs/chat-scroll-follow-intent/PR.md) | [fix.patch](prs/chat-scroll-follow-intent/fix.patch) |
| 17 | [fix(chat): report unaccepted message failures](prs/chat-send-rejection/PR.md) | [fix.patch](prs/chat-send-rejection/fix.patch) |
| 18 | [fix(chat): preserve drafts during speech recognition](prs/chat-speech-draft-ownership/PR.md) | [fix.patch](prs/chat-speech-draft-ownership/fix.patch) |
| 19 | [fix(chat): preserve submitted attachment ownership](prs/chat-submit-attachment-ownership/PR.md) | [fix.patch](prs/chat-submit-attachment-ownership/fix.patch) |
| 20 | [fix(chat): serialize pending submit hooks](prs/chat-submit-single-flight/PR.md) | [fix.patch](prs/chat-submit-single-flight/fix.patch) |
| 21 | [fix(chat): recover text from interrupted sends](prs/chat-unsent-draft-recovery/PR.md) | [fix.patch](prs/chat-unsent-draft-recovery/fix.patch) |
| 22 | [fix(chat): clean up failed response workers](prs/chat-worker-startup-cleanup/PR.md) | [fix.patch](prs/chat-worker-startup-cleanup/fix.patch) |
| 23 | [fix(import): preserve all snapshot provider configurations](prs/import-all-model-configs/PR.md) | [fix.patch](prs/import-all-model-configs/fix.patch) |
| 24 | [fix(import): move archived message bodies during persistence](prs/import-chat-body-copies/PR.md) | [fix.patch](prs/import-chat-body-copies/fix.patch) |
| 25 | [fix(import): preserve existing chats when snapshot IDs collide](prs/import-chat-id-collisions/PR.md) | [fix.patch](prs/import-chat-id-collisions/fix.patch) |
| 26 | [fix(import): preserve the current chat during snapshot migration](prs/import-current-chat-preference/PR.md) | [fix.patch](prs/import-current-chat-preference/fix.patch) |
| 27 | [fix(import): accept unwritten legacy function mappings](prs/import-default-function-mapping/PR.md) | [fix.patch](prs/import-default-function-mapping/fix.patch) |
| 28 | [fix(import): finish progress state when snapshot import fails](prs/import-failure-progress/PR.md) | [fix.patch](prs/import-failure-progress/fix.patch) |
| 29 | [fix(import): isolate imported files and workspace IDs](prs/import-file-workspace-collisions/PR.md) | [fix.patch](prs/import-file-workspace-collisions/fix.patch) |
| 30 | [fix(import): normalize self-closing legacy message tags](prs/import-legacy-empty-tags/PR.md) | [fix.patch](prs/import-legacy-empty-tags/fix.patch) |
| 31 | [fix(import): batch archived message part inserts](prs/import-message-part-batch/PR.md) | [fix.patch](prs/import-message-part-batch/fix.patch) |
| 32 | [fix(import): restore copied attachment paths in message content](prs/import-message-resource-paths/PR.md) | [fix.patch](prs/import-message-resource-paths/fix.patch) |
| 33 | [fix(import): preserve ObjectBox document nodes and content](prs/import-objectbox-documents/PR.md) | [fix.patch](prs/import-objectbox-documents/fix.patch) |
| 34 | [fix(import): decode standalone ObjectBox memory tag relations](prs/import-objectbox-tag-relations/PR.md) | [fix.patch](prs/import-objectbox-tag-relations/fix.patch) |
| 35 | [fix(import): preserve profiles without an ObjectBox database](prs/import-optional-memory-databases/PR.md) | [fix.patch](prs/import-optional-memory-databases/fix.patch) |
| 36 | [fix(import): accept snapshots without SQLite sidecars](prs/import-optional-wal/PR.md) | [fix.patch](prs/import-optional-wal/fix.patch) |
| 37 | [fix(import): preserve existing persona and memory identities](prs/import-persona-identity-collisions/PR.md) | [fix.patch](prs/import-persona-identity-collisions/fix.patch) |
| 38 | [fix(memory): preserve document nodes in portable backups](prs/import-portable-document-metadata/PR.md) | [fix.patch](prs/import-portable-document-metadata/fix.patch) |
| 39 | [fix(import): read released memory-space profile metadata](prs/import-profile-schema/PR.md) | [fix.patch](prs/import-profile-schema/fix.patch) |
| 40 | [fix(chat): preserve surviving reply variants after deletion](prs/import-reply-variants/PR.md) | [fix.patch](prs/import-reply-variants/fix.patch) |
| 41 | [fix(import): reuse the staged chat database for token usage](prs/import-reuse-staged-database/PR.md) | [fix.patch](prs/import-reuse-staged-database/fix.patch) |
| 42 | [fix(import): migrate intermediate Operit1 Room schemas](prs/import-room-schema-11-19/PR.md) | [fix.patch](prs/import-room-schema-11-19/fix.patch) |
| 43 | [fix(model): preserve fractional context limits in editors](prs/import-settings-context-precision/PR.md) | [fix.patch](prs/import-settings-context-precision/fix.patch) |
| 44 | [fix(chat): use the active context limit in token statistics](prs/import-settings-context-usage/PR.md) | [fix.patch](prs/import-settings-context-usage/fix.patch) |
| 45 | [fix(model): reject non-finite maximum context lengths](prs/import-settings-finite-context/PR.md) | [fix.patch](prs/import-settings-finite-context/fix.patch) |
| 46 | [fix(import): preserve Gemini Google Search settings](prs/import-settings-gemini-search/PR.md) | [fix.patch](prs/import-settings-gemini-search/fix.patch) |
| 47 | [fix(chat): run automatic summaries after ordinary send turns](prs/import-settings-ordinary-send-summary/PR.md) | [fix.patch](prs/import-settings-ordinary-send-summary/fix.patch) |
| 48 | [fix(import): retain unmigrated shared download paths](prs/import-shared-download-paths/PR.md) | [fix.patch](prs/import-shared-download-paths/fix.patch) |
| 49 | [fix(import): retain characters with stale fixed model selections](prs/import-stale-character-models/PR.md) | [fix.patch](prs/import-stale-character-models/fix.patch) |
| 50 | [fix(import): continue full snapshots with stale model selections](prs/import-stale-model-selection/PR.md) | [fix.patch](prs/import-stale-model-selection/fix.patch) |
| 51 | [fix(import): retain supported legacy TTS provider types](prs/import-tts-provider-types/PR.md) | [fix.patch](prs/import-tts-provider-types/fix.patch) |
| 52 | [fix(import): preview unsupported legacy providers](prs/import-unknown-provider-preview/PR.md) | [fix.patch](prs/import-unknown-provider-preview/fix.patch) |
| 53 | [fix(import): restore all owned user profile documents](prs/import-user-documents/PR.md) | [fix.patch](prs/import-user-documents/fix.patch) |
| 54 | [fix(chat): preserve and honor character model bindings](prs/model-fixed-character-binding/PR.md) | [fix.patch](prs/model-fixed-character-binding/fix.patch) |
| 55 | [fix(plugins): clear the agent timeout after send completion](prs/plugin-agent-timeout-cleanup/PR.md) | [fix.patch](prs/plugin-agent-timeout-cleanup/fix.patch) |
| 56 | [fix(flutter): bind plugin commands to shared core services](prs/plugin-flutter-core-command-executor/PR.md) | [fix.patch](prs/plugin-flutter-core-command-executor/fix.patch) |
| 57 | [fix(plugins): replay lifecycle hooks outside package manager locks](prs/plugin-lifecycle-replay-lock/PR.md) | [fix.patch](prs/plugin-lifecycle-replay-lock/fix.patch) |
| 58 | [fix(flutter): dispatch startup hooks after core node creation](prs/plugin-startup-after-core-node/PR.md) | [fix.patch](prs/plugin-startup-after-core-node/fix.patch) |
| 59 | [fix(plugins): rebuild cached TypeScript when output files are missing](prs/plugin-sync-missing-outputs/PR.md) | [fix.patch](prs/plugin-sync-missing-outputs/fix.patch) |
| 60 | [fix(plugins): identify the tool that failed in chat wrappers](prs/plugin-tool-error-label/PR.md) | [fix.patch](prs/plugin-tool-error-label/fix.patch) |
| 61 | [fix(claude): use one system prompt cache breakpoint](prs/provider-claude-system-cache-boundary/PR.md) | [fix.patch](prs/provider-claude-system-cache-boundary/fix.patch) |
| 62 | [fix(android): read snapshot documents off the main thread](prs/snapshot-input-background-io/PR.md) | [fix.patch](prs/snapshot-input-background-io/fix.patch) |
| 63 | [fix(flutter): preserve snapshot outcomes when temporary cleanup fails](prs/snapshot-ui-cleanup-errors/PR.md) | [fix.patch](prs/snapshot-ui-cleanup-errors/fix.patch) |
| 64 | [fix(flutter): stop snapshot import after settings disposal](prs/snapshot-ui-confirmation-mounted/PR.md) | [fix.patch](prs/snapshot-ui-confirmation-mounted/fix.patch) |
| 65 | [fix(flutter): show migration omissions in snapshot import results](prs/snapshot-ui-import-results/PR.md) | [fix.patch](prs/snapshot-ui-import-results/fix.patch) |
| 66 | [fix(flutter): show snapshot preparation progress before import preview](prs/snapshot-ui-preparation-progress/PR.md) | [fix.patch](prs/snapshot-ui-preparation-progress/fix.patch) |
| 67 | [fix(flutter): propagate watch opening failures without deadlocking](prs/startup-failed-watch-open/PR.md) | [fix.patch](prs/startup-failed-watch-open/fix.patch) |
| 68 | [fix(android): keep native bridge alive until process termination](prs/startup-native-bridge-lifetime/PR.md) | [fix.patch](prs/startup-native-bridge-lifetime/fix.patch) |
| 69 | [fix(android): avoid blocking lifecycle callbacks during runtime startup](prs/startup-nonblocking-lifecycle-events/PR.md) | [fix.patch](prs/startup-nonblocking-lifecycle-events/fix.patch) |
| 70 | [fix(flutter): keep onboarding open while a snapshot import is pending](prs/startup-onboarding-import-navigation/PR.md) | [fix.patch](prs/startup-onboarding-import-navigation/fix.patch) |
| 71 | [fix(flutter): retry onboarding completion without importing twice](prs/startup-onboarding-import-retry/PR.md) | [fix.patch](prs/startup-onboarding-import-retry/fix.patch) |
| 72 | [fix(flutter): open a restored conversation after onboarding import](prs/startup-open-imported-conversation/PR.md) | [fix.patch](prs/startup-open-imported-conversation/fix.patch) |
| 73 | [fix(flutter): open pairing events after storage is configured](prs/startup-pairing-after-storage/PR.md) | [fix.patch](prs/startup-pairing-after-storage/fix.patch) |
| 74 | [fix(android): restore storage roots before service runtime startup](prs/startup-restore-service-roots/PR.md) | [fix.patch](prs/startup-restore-service-roots/fix.patch) |
| 75 | [fix(android): release terminal state lock during command cancellation](prs/terminal-cancel-outside-global-state-lock/PR.md) | [fix.patch](prs/terminal-cancel-outside-global-state-lock/fix.patch) |
| 76 | [fix(android): retire terminal sessions after command execution errors](prs/terminal-cleanup-failed-command-execution/PR.md) | [fix.patch](prs/terminal-cleanup-failed-command-execution/fix.patch) |
| 77 | [fix(android): avoid repeated PRoot probes during terminal startup](prs/terminal-cold-start/PR.md) | [fix.patch](prs/terminal-cold-start/fix.patch) |
| 78 | [fix(android): cap collected terminal command output](prs/terminal-command-output-cap/PR.md) | [fix.patch](prs/terminal-command-output-cap/fix.patch) |
| 79 | [fix(android): retain PTY descriptor ownership during I/O](prs/terminal-descriptor-lifetime/PR.md) | [fix.patch](prs/terminal-descriptor-lifetime/fix.patch) |
| 80 | [fix(android): execute multiline terminal commands as one script](prs/terminal-execute-complete-scripts/PR.md) | [fix.patch](prs/terminal-execute-complete-scripts/fix.patch) |
| 81 | [fix(android): prepare exec arguments before forking the PTY](prs/terminal-fork-child/PR.md) | [fix.patch](prs/terminal-fork-child/fix.patch) |
| 82 | [fix(android): release terminal state while writing PTY input](prs/terminal-input-global-lock/PR.md) | [fix.patch](prs/terminal-input-global-lock/fix.patch) |
| 83 | [fix(android): serialize creation of named terminal sessions](prs/terminal-named-creation-race/PR.md) | [fix.patch](prs/terminal-named-creation-race/fix.patch) |
| 84 | [fix(android): preserve system shell command exit status](prs/terminal-preserve-system-shell-exit-status/PR.md) | [fix.patch](prs/terminal-preserve-system-shell-exit-status/fix.patch) |
| 85 | [fix(android): use the compatible PRoot startup environment](prs/terminal-proot-environment/PR.md) | [fix.patch](prs/terminal-proot-environment/fix.patch) |
| 86 | [fix(android): avoid signaling a reaped terminal process](prs/terminal-reaped-pid/PR.md) | [fix.patch](prs/terminal-reaped-pid/fix.patch) |
| 87 | [fix(android): retire timed-out sessions when shell recovery fails](prs/terminal-retire-unresponsive-timeouts/PR.md) | [fix.patch](prs/terminal-retire-unresponsive-timeouts/fix.patch) |
| 88 | [fix(android): serialize commands within each terminal session](prs/terminal-serialize-session-commands/PR.md) | [fix.patch](prs/terminal-serialize-session-commands/fix.patch) |
| 89 | [fix(android): recreate named terminals after their shell exits](prs/terminal-stale-session/PR.md) | [fix.patch](prs/terminal-stale-session/fix.patch) |

每个目录的 `metadata.json` 固定提交标题、分支名、基线及文件列表。完整目标文件可由 `fix.patch` 和基线还原；不重复保存本地 `changes.json` 的整文件副本。
