# AI Galgame 数据库设计

## 系统说明

- 用户可以拥有存档，系统会记录最后一次使用的角色。
- 剧本是外部资源文件夹，数据库中运行剧本则是记录玩家运行剧本到第几章节，每个章节都是由线性的事件events组成，所以会记录events的位置（int），以及变量信息比如玩家选择了分支之类的信息。
- 记忆仓库则是额外加到ai记忆里的，有关永久记忆的东西
- 台词首先由ai生成，然后会由程序处理，切割为tts语言，动作，情感，以及用深度学习模型预测后，显示立绘的情感，以及音频文件，主台词等各种信息，其中attribute则是代表台词的来源，比如是system还是assistant，还是user。额外的还会有角色的设计。
- 角色则是外部资源文件夹，数据库中记录角色的资源路径，以及角色名称。
- 台词可以有多个分支，和网页web聊天一样，ai的每次回复可以重新生成，或者从新的节点开始，所以设计了台词关系表

## 实体设计

- 角色：角色id，角色名称，资源路径
- 台词：台词id，原情绪，预测情绪，主内容，tts语言内容，动作内容，音频文件，属性（assistant, user, system）
- 存档：存档id，标题，创建日期，更新日期
- 用户：用户id，用户名，密码
- 运行剧本：剧本id，标题，变量信息（json），当前章节，事件序列
- 记忆仓库（MemoryBank）：记忆id，信息（长篇txt）

## 关系设计

- 用户和存档是一对多的关系，一个用户可以有多个存档，一个存档只有一个用户。
- 存档和剧本是一对一的关系，一个存档可以有一个运行中的剧本或者没有，一个运行中的剧本只能在一个存档中运行。
- 记忆仓库和存档是一对多的关系，一个存档可以有多个记忆仓库，一个记忆仓库只能属于一个存档。
- 台词和存档是一对多的关系，一个存档可以有多个台词，一个台词只能属于一个存档。
- 台词和角色是一对多的关系，一个角色可以有多个台词，一个台词只属于一个角色。
- 台词和台词是一对多的关系（父子关系，也就是“上一句台词的关系”），一句台词可以有多个下一句台词，但一个台词只能有一个上一句台词。
- 角色和记忆仓库是一对多的关系，一个角色可以有多个记忆仓库，一个记忆仓库只能属于一个角色。

## 数据表设计

角色表（role）：

- id：角色id，主键
- name：角色名称
- resource_folder：资源路径

台词表（line）：

- id：台词id，主键
- original_emotion：原情绪
- predicted_emotion：预测情绪
- content：主内容
- tts_content：tts语言内容
- action_content：动作内容
- audio_file: 音频文件
- attribute：属性（assistant, user, system）
- role_id：角色id，外键，引用角色表中的id
- script_role_id：剧本角色id，不是外键，剧本文件夹中对应的NPC的id，string类型
- display_name: 没有角色的情况下，显示的名称，在role_id和script_role_id都为空的时候，显示这个名称
- save_id：存档id，外键，引用存档表中的id
- parent_line_id：上一句台词id，外键，引用台词表中的id

存档表（save）：

- id：存档id，主键
- title：标题
- last_message_id: 最后一条消息id，外键，引用台词表中的id
- create_date：创建日期
- update_date：更新日期
- user_id：用户id，外键，引用用户表中的id
- running_script_id：剧本id，外键，引用剧本表中的id，可选，记录当前存档运行的是哪个运行剧本状态

用户表（user_info）：

- id：用户id，主键
- username：用户名
- password：密码
- last_character_id：最后使用的角色id，外键，引用角色表中的id

运行剧本表（running_script）：

- id：剧本id，主键
- script_folder: 剧本key，用于记录剧本的文件夹名称，确定唯一的剧本
- save_id: 存档id，外键，引用存档表中的id，记录存档的剧本运行状态
- variable_info：变量信息（json）
- current_chapter：当前章节(string)
- event_sequence：当前事件(int)

记忆仓库表（memory_bank）：

- id：记忆id，主键
- info：信息（json）
- save_id：存档id，外键，引用存档表中的id
- role_id：角色id，外键，引用角色表中的id
- script_roile_id：剧本角色id，不是外键，剧本文件夹中对应的NPC的id，string类型
