## 问题与复现

Claude 请求构造器给每个 system 文本块添加 `cache_control`。当角色、记忆等内容产生超过四个 system 块时，请求会超过 API 的显式缓存断点数量限制。复现条件是让 `system_parts` 至少包含五段文字并生成 Claude 请求。

## 原因与修复过程

逐块 `map` 无条件添加断点，而 `apply_stable_cache_breakpoints` 原先为空。移除逐块标记，在已组装的 system 数组末尾设置唯一缓存边界，仍缓存完整 system 前缀，保留用户自定义 system 末块已有的缓存设置。未加入旧版专用参数或改变其他供应商。

官方行为依据：[Claude prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)。

## 影响与验证

影响所有调用 Claude provider 的平台；不改存储或 host API。已核对源码调用关系及补丁基线；尚未独立编译，也未发送付费 API 请求。可检查五段 system 生成结果恰有一个断点作为针对性验证。

---
基线：`7fec1b2f17636c5b392c887fab28552219be807a`。此修复独立提交；已做源码核对和独立补丁应用检查，尚未独立编译或手机实测，因此以 Draft PR 提交。
