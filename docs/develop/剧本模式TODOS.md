## 剧本模式目前目标：

1. 实现【真心话大冒险】所涉及的游戏逻辑
2. 实现与前端的交互与 UI 功能表现

## 剧本模式所需功能 - 顶层

- [x] 定义旁白的 narration 消息 type，发送给前端
- [x] 读取 game_data/Script，获取剧本信息

## 关于 Script 结构的详细设计

- [x] Script/story_config.yaml 用于存放剧本基础信息相关内容
- [x] Script/Assets 用于存放游戏需要使用的辅助资源内容
- [x] Script/Assets/CGs 用于存放游戏需要用的 CG 相关内容
- [x] Script/Assets/Backgrounds 用于存放游戏需要用的背景相关内容
- [x] Script/Assets/SoundEffects 用于存放游戏需要用的音效资源
- [x] Script/Assets/BGMs 用于存放游戏播放的背景音乐
- [x] Script/Characters 存放剧本游戏里设定各个人物的信息，结构与普通 Character 一致
- [x] Script/Chapters 用于定义剧本文件的详细驱动内容

```
📂 game_data/Script/
├── 📁 剧本一
├── 📁 剧本二
│   ├─ 📁 Assets/
│   │  ├── 📁 Backgrounds/
│   │  ├── 📁 CGs/
│   │  ├── 📁 SoundEffects/
│   │  └── 📁 BGMs/
│   ├─ 📁 Characters/
│   │  └── 📄 保留和之前一样的设定
│   ├─ 📁 Script/
│   │  ├── 📄 Chapter_1.yaml
│   │  └── 📄 Chapter_2.yaml
│   └─ 📄 story_config.yaml
└── 📁 (其他剧本...)
```

- 其中，关于 config.yaml 的详细内容：

```
# story_config.yaml
# 剧本全局配置文件

script_name: "SnowWind_Get_Fucked_Script"
start_chapter_id: "Chapter_Intro_1.yaml"
description: "这是一个关于风雪如何因为冷暴力然后艾草的剧本"
```

## 关于对 Script 读取的流程

1. 程序在初始化的时候，读取 data/game_data/Script 文件夹
2. 遍历 Script 文件夹中的每一个子文件夹，确定是否有剧本（通过 config.yaml 识别）
3. 程序自动为 Scripts 排序，形成一个剧本列表 List 可供选择，每个剧本的 id 使用递增的方法形成（日后会使用数据库存储）
4. 通过调用 Script_Manager.start_script(id)，开始剧本的进行流程

## 关于对 Script 进行流程的详细设计 Script_Manager

### 1. 初始化阶段

1. 读取所有 Characters，为他们每一个人初始化一个 memory 数组:

```
for charater in Function.read_script_characters(self.script_name):
    characters_memory[character.name] = init_character_memory(character.settings.prompt)
```

2. 通过读取 story.config.yaml，确定第一个进入的章节，然后转交给 Chapter_Manager 处理后事

```
intro_chapter:str = self.get_first_chapter()
chapter_manager.process_chapter(intro_chapter)
```

### 2. 剧本演绎阶段

#### 单个 `Chapter` 总述

- 单个剧本示范如下面的代码所示：

```
Events:
  - "Type: Narration | Mode: Preset": 又到了愉快的周末~ 你来到了灵灵的家里，一起寻思着怎么消遣周末的愉快时光~。
  - "Type: Dialogue | Character: participant_1 | Mode: Preset": “哎，{player_name}，最近猫娘发电群一直在玩的真心话大冒险似乎挺有意思的，要不要一起玩呀！”
  - "Type: Player | Mode: Preset": 唔，那我可要好好探索你的小心灵了哦~
  - "Type: Narration | Mode: Preset": 灵灵踩着轻盈的脚步拿来了两个骰子，灵巧地扔在了莱姆的手上。
  - "Type: Dialogue | Character: participant_1 | Mode: Preset": “来叭，我和 {player_name} 的终极对决！”

EndCondition:
  Type: Linear
  NextUnitID: 01_Start_Turn
```

- 其中有两个重要的元素：`Events` 与 `EndCondition`

#### `Events`逻辑

1. `Events` 的 `Key` 用于定义本次事件的类型, `Value` 用于作为本次事件的输入，所以设计`Events_Handler`专门用于处理单个事件的解析
2. 目前需要的`Events_Handler`处理能力目标包括：
   - 对旁白，玩家，单个 AI 角色的强制叙述控制
   - 玩家输入事件的实现
   - 旁白，单个 AI 角色对玩家输入时间或剧本输入事件+响应

#### `EndCondition`逻辑

1. `EndCondition`必须包含`Type`和`Next_Chapter`两个必要的属性，可以设计`End_Handler`专门用于处理
2. 目前需要`End_Handler`处理的能力包括
   - 对`Type: Linear`线性事件的识别
   - 对`Next_Chapter`的识别以及过度

#### 剧本演绎总体逻辑（伪代码）

```
intro_chapter:str = Script_Manager.get_first_chapter()

while True:
    next_chapter:str = Chapter_Manager.start_chartper(intro_chapter)
    if next_chapter is None:
        break

# Chapter_Manager 逻辑
def Chapter_Manager.start_chartper(chapter) -> str:
    self.events_list:list[dict] = self.get_chapter_events
    self.end: self.get_chapter_end
    while True:
        current_event = self.events_list.pop() # 出队一个event
        Event_Handler.process_event(current_event)
        if self.events_list.empty(): break
    next_chapter = End_Handler.process_end(end)
    return next_chapter

# Events_Handler 逻辑
def Event_Handler.process_event(current_event) -> dict:
    logic = current_event.key()
    content = current_event.value()
    # 读取events的逻辑...
    character = 处理好后，决定影响的对象，对于旁白如下
    message_broker.enqueue_message(
        self.client_id,
        "旁白" | character,
        "{更新剧本对话：风雪：我喜欢你灵灵，结婚吧; 钦灵:哎？这么突然的吗？本次任务：描述灵灵羞怯的样子}"
    )
    # 注意，这里的角色和后面的提示词，都是由读取events的逻辑实现的
    # 由于这里逻辑实现复杂，不继续写了，之后可以参考NeoChat的

# End_Handler 实现
# 读取Type为Linear，返回Next_Chapter即可
# 我建议如果当前剧本章节过长，或者和下一阶段有重大区别，可以在这里添加额外的，对角色记忆压缩处理的功能，总结这一章节发生的内容加入到角色的记忆中，比RAG会好很多
```

### 测试:

- [x] 能够读取所有剧本文件夹
- [x] 能够读取剧本信息与它对应的第一个章节位置
- [x] 能够读取章节的所有事件
- [x] 能够正确进行事件并且在最后一个检测出结束
- [x] 能够正确读取最终条件
- [x] 能够正确跳转到下一个剧本
