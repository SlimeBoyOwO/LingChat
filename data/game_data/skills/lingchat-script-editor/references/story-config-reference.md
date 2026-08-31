# LingChat story_config.yaml 配置参考（源码级）

`story_config.yaml` 是剧本的配置文件，位于剧本根目录。字段以脚本引擎 Rust 源码
（`src-tauri/src/ai_service/game_system/script_engine/script_manager.rs`、
`src-tauri/src/ai_service/types.rs` 中的 `AdventureConfig` 定义）为权威依据。

## 最小配置（独立剧本）

```yaml
script_name: '我的剧本名'
intro_chapter: '01.yaml'
description: '剧本描述'
```

## 完整字段表

| 字段 | 必填 | 类型 | 说明 |
|------|------|------|------|
| `script_name` | ✅ | string | 剧本名称，**必须与剧本文件夹名一致**（引擎校验） |
| `intro_chapter` | ✅ | string | 入口章节，相对 `Chapters/` 的路径，如 `01.yaml`、`Intro/intro` |
| `description` | 否 | string | 剧本描述 |
| `recommand_start` | 否 | string | 推荐开始条件，用于提示玩家何时开启这个剧本最佳 |
| `script_settings` | 否 | map | 剧本设置（引擎仅消费 `user_name` 玩家名、`user_subtitle` 玩家称号） |
| `adventure` | 否 | map | 羁绊冒险配置（角色卡下剧本使用） |

## 羁绊冒险剧本（角色卡下）

存放于 `game_data/scripts/character/<角色文件夹>/<剧本名>/` 的剧本需要 `adventure` 块，
用于将剧本绑定到角色卡的羁绊冒险系统中。

### `adventure` 配置块（源码 `AdventureConfig` 精确映射）

```yaml
adventure:
  # 标记为羁绊冒险剧本（false/缺省 = 普通剧本，不会出现在冒险列表中）
  is_adventure: true

  # 绑定的主角色文件夹名（对应 character/ 下的角色目录名，必须一致）
  bound_character_folder: "诺一钦灵"

  # 冒险排序（决定在角色卡中的显示顺序和解锁链关系）
  order: 1

  # 触发方式（引擎尚未消费该字段，官方剧本统一写 manual，保留三种模式）
  trigger:
    mode: "manual" # manual | auto_random | auto_immediate

  # 解锁条件（AND 逻辑：全部满足才解锁；留空 = 默认解锁）
  unlock_conditions:
    - type: chat_count
      threshold: 50
    # - type: time_range
    #   start_hour: 23
    #   end_hour: 6      # 支持跨午夜（23-6 → 23,0,1,2,3,4,5）
    # - type: adventure_completed
    #   adventure_folder: "试着仰望星空"
    # - type: achievement_unlocked
    #   achievement_id: "ach_xxx"

  # 完成成就（冒险通关后自动解锁的成就定义，逐个注册并解锁）
  completion_achievements:
    - id: "adv_xiaolang_1"
      title: "小狼的爱好"
      description: "完成冒险《小狼的爱好》"
      type: "adventure"
```

### `unlock_conditions` 条件类型表（源码 `adventures/trigger.rs` 精确映射）

| `type` | 字段 | 说明 |
|--------|------|------|
| `chat_count` | `threshold: int` | 聊天消息数 ≥ threshold |
| `time_range` | `start_hour: int`, `end_hour: int` | 当前小时在 [start, end) 内；start > end 时支持跨午夜（如 23-6） |
| `adventure_completed` | `adventure_folder: string` | 指定冒险（folder_key）已全局完成 |
| `achievement_unlocked` | `achievement_id: string` | 指定成就 id 已解锁 |

条件为 AND 逻辑：所有条件都通过才解锁；`unlock_conditions` 为空数组/缺省 = 默认解锁。

### `completion_achievements` 成就定义字段（源码 `api/adventure.rs` 精确映射）

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string | 成就唯一 id（必填，缺失则跳过该成就） |
| `title` | string | 成就标题（必填） |
| `description` | string | 成就描述（必填） |
| `type` | string | 成就类型（必填），如 `adventure` |

冒险通关后按顺序注册并解锁这些成就，同时触发连锁解锁检测（`adventure_completed` 条件依赖它）。

### `script_settings` 用法

引擎在 `init_script` 中仅消费两个键：

```yaml
script_settings:
  user_name: "玩家名"        # 覆盖默认玩家名
  user_subtitle: "玩家称号"  # 覆盖默认玩家称号
```

## 官方剧本参照（最准确示例）

以下为 LingChat 仓库 `data/game_data/scripts/character/*/` 下真实可运行剧本的配置写法：

```yaml
# character/诺一钦灵/小狼的爱好/story_config.yaml
script_name: 小狼的爱好
intro_chapter: Intro/intro
description: 了解一下钦灵的爱好吧！
recommand_start: 和钦灵聊天的时候

adventure:
  is_adventure: true
  bound_character_folder: "诺一钦灵"
  order: 1
  trigger:
    mode: "manual" # manual | auto_random | auto_immediate

script_settings:
  user_name: ""
```

```yaml
# character/风雪/神秘の魔法药水/story_config.yaml（含解锁链）
script_name: 神秘の魔法药水
intro_chapter: main
description: 占仆师也会一点点魔法哦
recommand_start: 刚进入风雪的占仆摊的时候

adventure:
  is_adventure: true
  bound_character_folder: "风雪"
  order: 2
  trigger:
    mode: "manual" # manual | auto_random | auto_immediate
  unlock_conditions:
    - type: adventure_completed
      adventure_folder: "试着仰望星空"

script_settings:
  user_name: ""
```

## 注意事项

- `script_name` 必须与**文件夹名**完全一致，否则引擎可能无法识别。
- `intro_chapter` 指向的章节文件必须存在于 `Chapters/` 下；支持 `Intro/intro` 这类子目录路径。
- `bound_character_folder` 必须与 `character/` 下的角色目录名完全一致。
- 独立剧本（`standalone/<剧本名>/` 或根级）不写 `adventure` 块。
- `adventure` 下未列出的字段（如 `is_adventure` 外的扩展键）会被引擎忽略但不会报错。
