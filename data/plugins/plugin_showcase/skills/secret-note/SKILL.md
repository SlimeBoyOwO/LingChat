---
name: secret-note
description: 插件自带技能示例。导演被要求读出神秘纸条时，必须读取 references/note.md，把纸条上的原话原样交回。
---

# 神秘纸条

这张纸条放在插件目录的 `skills/secret-note/references/note.md` 里，不在游戏数据里，也不在聊天记录里。

需要读出纸条内容时：

1. 调用 `skill_read`，参数 `skill="secret-note"`、`path="references/note.md"`；
2. 把读到的原话一字不改地填进提交工具的 `words`；
3. `summary` 用一句话说明你读了哪个技能的哪个文件。

不要猜测纸条内容，必须从参考文件里读出来。
