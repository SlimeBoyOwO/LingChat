# LingChat 剧本角色设定参考

剧本专属 NPC 放在 `<剧本目录>/characters/<角色文件夹>/settings.yml`。
字段与普通角色卡一致，唯多加一个 **`script_role_key`** 作为剧本内唯一 id。

## 最小用法

```yaml
ai_name: 风雪
ai_subtitle: LingChat Studio
body_part: null
bubble_left: 25
bubble_top: 5
character_folder: 风雪

# 剧本内唯一 id（在剧本事件中通过该 id 调用角色）
script_role_key: snow_wind
```

## 在剧本中调用

```yaml
- type: modify_character
  action: show_character
  character: snow_wind       # ← 即 script_role_key
  emotion: 认真
  duration: 1.5

- type: ai_dialogue
  character: snow_wind
  prompt: 风雪闻到隔壁传来的泡面味
```

## 常用字段说明（参照官方角色 `诺一钦灵/settings.yml`）

| 字段 | 说明 |
|------|------|
| `ai_name` | 角色显示名 |
| `ai_subtitle` | 副标题 |
| `character_folder` | 角色资源文件夹名 |
| `script_role_key` | **剧本内唯一 id**（必加） |
| `system_prompt` | 系统提示（人设） |
| `system_prompt_example` | 人设示例对话 |
| `info` | 角色信息 |
| `clothes` | 服装（配合 `modify_character.clothes` 使用） |
| `user_name` | 对玩家的称呼 |
| `body_part` / `bubble_left` / `bubble_top` | 演出布局参数 |

> 完整字段以角色卡系统源码与官方角色卡为最准确参照。编写剧本 NPC 时，最简单的方式是
> 复制一个官方角色卡，加上 `script_role_key` 字段即可。
