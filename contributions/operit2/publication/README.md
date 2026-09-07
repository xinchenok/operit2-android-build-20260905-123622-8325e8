# 89个上游修复PR已提交

**89个独立修复PR全部提交成功，当前均为open Draft，编号#8–#96。** 逐项核对了预定分支、提交号和目标仓库，没有遗漏或重复。

[成功的发布任务](https://github.com/xinchenok/Operit2/actions/runs/34119116134) · [完整结果JSON](results.json)

每个PR有独立修复提交和修复说明。尚未独立编译或手机实测，也未合并；没有触发APK构建。

| PR | 修复说明 | 提交号 |
|---|---|---|
| [#8](https://github.com/AAswordman/Operit2/pull/8) | fix(chat): initialize the first conversation | `758c163` |
| [#9](https://github.com/AAswordman/Operit2/pull/9) | fix(chat): respect cancelled bottom following | `ba49dd9` |
| [#10](https://github.com/AAswordman/Operit2/pull/10) | fix(android): restore storage roots before service runtime startup | `84cac02` |
| [#11](https://github.com/AAswordman/Operit2/pull/11) | fix(android): keep native bridge alive until process termination | `f28360a` |
| [#12](https://github.com/AAswordman/Operit2/pull/12) | fix(flutter): keep onboarding open while a snapshot import is pending | `f0e9253` |
| [#13](https://github.com/AAswordman/Operit2/pull/13) | fix(flutter): retry onboarding completion without importing twice | `7c2b5f5` |
| [#14](https://github.com/AAswordman/Operit2/pull/14) | fix(flutter): open a restored conversation after onboarding import | `57916d1` |
| [#15](https://github.com/AAswordman/Operit2/pull/15) | fix(android): read snapshot documents off the main thread | `f074ea6` |
| [#16](https://github.com/AAswordman/Operit2/pull/16) | fix(flutter): show snapshot preparation progress before import preview | `edd50c4` |
| [#17](https://github.com/AAswordman/Operit2/pull/17) | fix(import): continue full snapshots with stale model selections | `4ffade8` |
| [#18](https://github.com/AAswordman/Operit2/pull/18) | fix(import): accept unwritten legacy function mappings | `18c8516` |
| [#19](https://github.com/AAswordman/Operit2/pull/19) | fix(import): batch archived message part inserts | `f4cc617` |
| [#20](https://github.com/AAswordman/Operit2/pull/20) | fix(import): reuse the staged chat database for token usage | `25e9c8b` |
| [#21](https://github.com/AAswordman/Operit2/pull/21) | fix(import): move archived message bodies during persistence | `f769f08` |
| [#22](https://github.com/AAswordman/Operit2/pull/22) | fix(android): use the compatible PRoot startup environment | `7859d79` |
| [#23](https://github.com/AAswordman/Operit2/pull/23) | fix(android): avoid repeated PRoot probes during terminal startup | `f806d7e` |
| [#24](https://github.com/AAswordman/Operit2/pull/24) | fix(android): retain PTY descriptor ownership during I/O | `a70b7db` |
| [#25](https://github.com/AAswordman/Operit2/pull/25) | fix(chat): separate attachment cleanup from send errors | `2581dd7` |
| [#26](https://github.com/AAswordman/Operit2/pull/26) | fix(chat): cancel stale message locator jumps | `323ccbe` |
| [#27](https://github.com/AAswordman/Operit2/pull/27) | fix(chat): guard detached scroll controllers | `73bf16a` |
| [#28](https://github.com/AAswordman/Operit2/pull/28) | fix(chat): stop group planning and pending member turns on cancellation | `56bf3b8` |
| [#29](https://github.com/AAswordman/Operit2/pull/29) | fix(chat): release the core command lock during group responses | `e941237` |
| [#30](https://github.com/AAswordman/Operit2/pull/30) | fix(chat): resolve group orchestration from the target conversation | `4b6b1dc` |
| [#31](https://github.com/AAswordman/Operit2/pull/31) | fix(chat): preserve drafts changed during submit hooks | `91ba42c` |
| [#32](https://github.com/AAswordman/Operit2/pull/32) | fix(markdown): assign unique paragraph break keys | `f4ccdb9` |
| [#33](https://github.com/AAswordman/Operit2/pull/33) | fix(chat): ignore nested scroll notifications | `0abd15c` |
| [#34](https://github.com/AAswordman/Operit2/pull/34) | fix(chat): retain partial answers after stream failures | `c1eaad8` |
| [#35](https://github.com/AAswordman/Operit2/pull/35) | fix(chat): preserve edits while loading queued input | `a4ee0c2` |
| [#36](https://github.com/AAswordman/Operit2/pull/36) | fix(chat): restore queued input rejected by hooks | `f0f91ae` |
| [#37](https://github.com/AAswordman/Operit2/pull/37) | fix(chat): keep draft attachments out of queued sends | `ed344d9` |
| [#38](https://github.com/AAswordman/Operit2/pull/38) | fix(chat): retain the submitted reply target | `86fcd9c` |
| [#39](https://github.com/AAswordman/Operit2/pull/39) | fix(chat): report unaccepted message failures | `821ed3d` |
| [#40](https://github.com/AAswordman/Operit2/pull/40) | fix(chat): preserve drafts during speech recognition | `821dff9` |
| [#41](https://github.com/AAswordman/Operit2/pull/41) | fix(chat): preserve submitted attachment ownership | `f04d1c9` |
| [#42](https://github.com/AAswordman/Operit2/pull/42) | fix(chat): serialize pending submit hooks | `62b6ed5` |
| [#43](https://github.com/AAswordman/Operit2/pull/43) | fix(chat): recover text from interrupted sends | `a64123b` |
| [#44](https://github.com/AAswordman/Operit2/pull/44) | fix(chat): clean up failed response workers | `c3dcd8c` |
| [#45](https://github.com/AAswordman/Operit2/pull/45) | fix(import): preserve all snapshot provider configurations | `5c5b54a` |
| [#46](https://github.com/AAswordman/Operit2/pull/46) | fix(import): preserve existing chats when snapshot IDs collide | `1bdef5e` |
| [#47](https://github.com/AAswordman/Operit2/pull/47) | fix(import): preserve the current chat during snapshot migration | `f97117e` |
| [#48](https://github.com/AAswordman/Operit2/pull/48) | fix(import): finish progress state when snapshot import fails | `a1e0c0b` |
| [#49](https://github.com/AAswordman/Operit2/pull/49) | fix(import): isolate imported files and workspace IDs | `671d6cc` |
| [#50](https://github.com/AAswordman/Operit2/pull/50) | fix(import): normalize self-closing legacy message tags | `73d659d` |
| [#51](https://github.com/AAswordman/Operit2/pull/51) | fix(import): restore copied attachment paths in message content | `199dab2` |
| [#52](https://github.com/AAswordman/Operit2/pull/52) | fix(import): preserve ObjectBox document nodes and content | `7e2f965` |
| [#53](https://github.com/AAswordman/Operit2/pull/53) | fix(import): decode standalone ObjectBox memory tag relations | `e62c612` |
| [#54](https://github.com/AAswordman/Operit2/pull/54) | fix(import): preserve profiles without an ObjectBox database | `2ff21f7` |
| [#55](https://github.com/AAswordman/Operit2/pull/55) | fix(import): accept snapshots without SQLite sidecars | `2408a0e` |
| [#56](https://github.com/AAswordman/Operit2/pull/56) | fix(import): preserve existing persona and memory identities | `46fea5c` |
| [#57](https://github.com/AAswordman/Operit2/pull/57) | fix(memory): preserve document nodes in portable backups | `f785a7c` |
| [#58](https://github.com/AAswordman/Operit2/pull/58) | fix(import): read released memory-space profile metadata | `42af120` |
| [#59](https://github.com/AAswordman/Operit2/pull/59) | fix(chat): preserve surviving reply variants after deletion | `57ca35b` |
| [#60](https://github.com/AAswordman/Operit2/pull/60) | fix(import): migrate intermediate Operit1 Room schemas | `e54a263` |
| [#61](https://github.com/AAswordman/Operit2/pull/61) | fix(model): preserve fractional context limits in editors | `3713358` |
| [#62](https://github.com/AAswordman/Operit2/pull/62) | fix(chat): use the active context limit in token statistics | `42c7380` |
| [#63](https://github.com/AAswordman/Operit2/pull/63) | fix(model): reject non-finite maximum context lengths | `26aca75` |
| [#64](https://github.com/AAswordman/Operit2/pull/64) | fix(import): preserve Gemini Google Search settings | `a2941e7` |
| [#65](https://github.com/AAswordman/Operit2/pull/65) | fix(chat): run automatic summaries after ordinary send turns | `4fa1095` |
| [#66](https://github.com/AAswordman/Operit2/pull/66) | fix(import): retain unmigrated shared download paths | `280125a` |
| [#67](https://github.com/AAswordman/Operit2/pull/67) | fix(import): retain characters with stale fixed model selections | `b8b228c` |
| [#68](https://github.com/AAswordman/Operit2/pull/68) | fix(import): retain supported legacy TTS provider types | `bb384f3` |
| [#69](https://github.com/AAswordman/Operit2/pull/69) | fix(import): preview unsupported legacy providers | `470c381` |
| [#70](https://github.com/AAswordman/Operit2/pull/70) | fix(import): restore all owned user profile documents | `154b968` |
| [#71](https://github.com/AAswordman/Operit2/pull/71) | fix(chat): preserve and honor character model bindings | `32d8d09` |
| [#72](https://github.com/AAswordman/Operit2/pull/72) | fix(plugins): clear the agent timeout after send completion | `39b1cd9` |
| [#73](https://github.com/AAswordman/Operit2/pull/73) | fix(flutter): bind plugin commands to shared core services | `abca9ec` |
| [#74](https://github.com/AAswordman/Operit2/pull/74) | fix(plugins): replay lifecycle hooks outside package manager locks | `9c1160c` |
| [#75](https://github.com/AAswordman/Operit2/pull/75) | fix(flutter): dispatch startup hooks after core node creation | `224eb03` |
| [#76](https://github.com/AAswordman/Operit2/pull/76) | fix(plugins): rebuild cached TypeScript when output files are missing | `ef273b2` |
| [#77](https://github.com/AAswordman/Operit2/pull/77) | fix(plugins): identify the tool that failed in chat wrappers | `2965e64` |
| [#78](https://github.com/AAswordman/Operit2/pull/78) | fix(claude): use one system prompt cache breakpoint | `f49c161` |
| [#79](https://github.com/AAswordman/Operit2/pull/79) | fix(flutter): preserve snapshot outcomes when temporary cleanup fails | `baca45b` |
| [#80](https://github.com/AAswordman/Operit2/pull/80) | fix(flutter): stop snapshot import after settings disposal | `6767429` |
| [#81](https://github.com/AAswordman/Operit2/pull/81) | fix(flutter): show migration omissions in snapshot import results | `e8618fa` |
| [#82](https://github.com/AAswordman/Operit2/pull/82) | fix(flutter): propagate watch opening failures without deadlocking | `ef8efe2` |
| [#83](https://github.com/AAswordman/Operit2/pull/83) | fix(android): avoid blocking lifecycle callbacks during runtime startup | `a253e52` |
| [#84](https://github.com/AAswordman/Operit2/pull/84) | fix(flutter): open pairing events after storage is configured | `0a7f45d` |
| [#85](https://github.com/AAswordman/Operit2/pull/85) | fix(android): release terminal state lock during command cancellation | `b12b09c` |
| [#86](https://github.com/AAswordman/Operit2/pull/86) | fix(android): retire terminal sessions after command execution errors | `de00eeb` |
| [#87](https://github.com/AAswordman/Operit2/pull/87) | fix(android): cap collected terminal command output | `c6b849c` |
| [#88](https://github.com/AAswordman/Operit2/pull/88) | fix(android): execute multiline terminal commands as one script | `c8cae1e` |
| [#89](https://github.com/AAswordman/Operit2/pull/89) | fix(android): prepare exec arguments before forking the PTY | `5334d1b` |
| [#90](https://github.com/AAswordman/Operit2/pull/90) | fix(android): release terminal state while writing PTY input | `d5cfcd8` |
| [#91](https://github.com/AAswordman/Operit2/pull/91) | fix(android): serialize creation of named terminal sessions | `112490f` |
| [#92](https://github.com/AAswordman/Operit2/pull/92) | fix(android): preserve system shell command exit status | `96ad5bd` |
| [#93](https://github.com/AAswordman/Operit2/pull/93) | fix(android): avoid signaling a reaped terminal process | `2b6a144` |
| [#94](https://github.com/AAswordman/Operit2/pull/94) | fix(android): retire timed-out sessions when shell recovery fails | `7955196` |
| [#95](https://github.com/AAswordman/Operit2/pull/95) | fix(android): serialize commands within each terminal session | `9ef9fba` |
| [#96](https://github.com/AAswordman/Operit2/pull/96) | fix(android): recreate named terminals after their shell exits | `064d612` |

[角色供应商导入衔接补丁](../dependencies/import-character-provider-mapping/README.md)需要前置修复合入后处理，不属于这89个可独立提交的PR。
