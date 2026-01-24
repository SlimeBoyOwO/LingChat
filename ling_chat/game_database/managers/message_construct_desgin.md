# 记忆构建器设计

## 功能描述

记忆构建器（Memory Builder）是一个算法，用于将数据库内的台词和记忆，构建为给LLM模型使用的记忆内容。

## 基础信息

### Line（台词）的属性包括：

```python
class Line(SQLModel, table=True):
    """台词表：核心对话树结构"""
    __tablename__ = "line"

    id: Optional[int] = Field(default=None, primary_key=True)

    # 情绪与内容
    original_emotion: Optional[str] = None
    predicted_emotion: Optional[str] = None
    content: str
    tts_content: Optional[str] = None
    action_content: Optional[str] = None
    audio_file: Optional[str] = None

    attribute: str # assistant, user, system

    # 角色逻辑
    role_id: Optional[int] = Field(default=None, foreign_key="role.id", nullable=True)
    script_role_id: Optional[str] = None # 剧本NPC ID (String)
    display_name: Optional[str] = None   # 兜底显示名称

    # 外键关系
    save_id: int = Field(foreign_key="save.id")

    # 自引用外键：构建对话树
    parent_line_id: Optional[int] = Field(default=None, foreign_key="line.id", nullable=True)
```

### 对于 LLM，消息存储的规则如下：

```python
self.memory = [
            {
                "role": "system",
                "content": self.ai_prompt
            },
            {
                "role": "user",
                "content": "你好呀"
            },
            {
                "role": "assistant",
                "content": "【高兴】莱姆你来啦（开心的摇尾巴）【好奇】你今天想聊什么呀？"
            }
        ]
```

## 构建出的给AI的记忆信息规则

- 台词会通过下面的数据库查询，通过`list`的方式返回，相关代码如下：

```python
@staticmethod
def get_chat_history(save_id: int) -> List[Line]:
    """
    获取完整对话历史，从根节点到 last_message_id
    """
    with Session(engine, expire_on_commit=False) as session:
        save = session.get(Save, save_id)
        if not save or not save.last_message_id:
            return []

        # 获取该存档所有台词
        statement = select(Line).where(Line.save_id == save_id)
        lines = session.exec(statement).all()
        lines_map = {line.id: line for line in lines}

        history = []
        current_id = save.last_message_id

        while current_id:
            line = lines_map.get(current_id)
            if not line:
                break
            history.append(line)
            current_id = line.parent_line_id

        return list(reversed(history))
```

- 基础台词的记忆构造规则如下（字符串），其中在构造记忆的时候，会提示：

1. 当attribute为`user`时，构造规则为：`role`: `user`, `content`: `line.content`

   > 当attribute为`user`的时候，一般是没有`tts_content`,`emotion`或者`action_content`字段的，所以不需要构建

2. 当attribute为`system`时，且role_id或script_role_id相同或display_name相同，基础构造规则为：`role`: `system`, `content`: `line.content`，如果不同则忽视这一部分，不构造到记忆中去

3. 当attribute为`assistant`时，基础构造规则为：`role`: `user/assistant，根据下面规则而定`, `content`: `【line.original_emotion】line.content<line.tts_content>（line.action_content）`

   > 其中，如果original_emotion为空，则不显示大括号（中文大括号），如果action_content为空，则不显示括号（中文括号）

- 存在连续多个assistant，且role_id或script_role_id相同或display_name相同，则合并成一条消息，构造规则为：`role`: `assistant`, `【line.original_emotion】line.content<line.tts_content>（line.action_content）【line.original_emotion】line.content<line.tts_content>（line.action_content）...`

- 如果存在连续多个assistant，但role_id或script_role_id不同或display_name不同，则应该把连续的这种情况的消息，作为`user`消息，并用大括号括起来，构造规则为：`role`: `user`, `content`: `{line.display_name: 【line.original_emotion】line.content（line.action_content）\nline.display_name: 【line.original_emotion】line.content（line.action_content)...}`
  > 特别的，在这种情况下，假如下一句是user，且下一句的下一句是不一样的role_id或script_role_id不同或display_name不同，则也构造到这个部分中（也就是大括号内部），构造规则为：`role`: `user`, `content`: `{assistant_line.display_name: 【assistant_line.original_emotion】assistant_line.content（assistant_line.action_content）\nuser_line.display_name: user_line.content\nassistant_line.display_name: 【assistant_line.original_emotion】assistant_line.content（assistant_line.action_content）...}`
  > 但假如下一句是user，或者是出现连续多个user，且下一句的下一句是assistant且role_id或script_role_id相同或display_name相同，则不构造到这个部分中（也就是大括号内部），而是构造到大括号外面，构造规则为：`role`: `user`, `content`: `{assistant_line.display_name: 【assistant_line.original_emotion】assistant_line.content（assistant_line.action_content）} \n last_user_content current_user_content`
  > 这是为了让 AI 记忆构建中，让AI专注于最后一个user消息，而大括号中的内容只是作为剧情和环境补充信息

## 示范

### 1. 基础场景，AI与玩家正常一对一对话。

返回的消息列表为（部分属性）：

```json
[
  {
    "line_id": 1,
    "attribute": "system",
    "content": "你叫钦灵，进行角色扮演"
  },
  {
    "line_id": 2,
    "attribute": "user",
    "content": "你好呀钦灵",
    "display_name": "莱姆"
  },
  {
    "line_id": 3,
    "attribute": "assistant",
    "original_emotion": "开心",
    "content": "你好呀莱姆，怎么今天来找我玩啦",
    "tts_content": "こんにちは、ライム、今日はどうして遊びに来たの",
    "action_content": "轻轻地摇了摇尾巴",
    "display_name": "钦灵"
  },
  {
    "line_id": 4,
    "attribute": "assistant",
    "original_emotion": "好奇",
    "content": "怎么样，今天的作业写完了吗？",
    "tts_content": "どうだ、今日の宿題は終わった？",
    "action_content": None,
    "display_name": "钦灵"
  },
  {
    "line_id": 5,
    "attribute": "assistant",
    "original_emotion": "担心",
    "content": "新的老师可严格了听说...",
    "tts_content": "新しい先生は聞いた通り、厳しいです...",
    "action_content": None,
    "display_name": "钦灵"
  },
  {
    "line_id": 6,
    "attribute": "user",
    "content": "没事啦，昨天已经搞定了",
    "display_name": "莱姆"
  },
  {
    "line_id": 7,
    "attribute": "assistant",
    "original_emotion": "无奈",
    "content": "呜呜，我还没写完呢...",
    "tts_content": "うう、まだ書き終わってないの...",
    "action_content": "轻轻地叹了口气",
    "display_name": "钦灵"
  },
  {
    "line_id": 8,
    "attribute": "assistant",
    "original_emotion": "调皮",
    "content": "那个，你的作业给我看看怎么样呀？",
    "tts_content": "あの、あなたの宿題を見てどうですか？",
    "action_content": None,
    "display_name": "钦灵"
  },
]
```

记忆构建器将根据这些消息构建一个记忆，并返回一个包含记忆的字典。

```python
self.result_memory = [
            {
                "role": "system",
                "content": "你叫钦灵，进行角色扮演"
            },
            {
                "role": "user",
                "content": "你好呀钦灵"
            },
            {
                "role": "assistant",
                "content": "【开兴】你好呀莱姆，怎么今天来找我玩啦<こんにちは、ライム、今日はどうして遊びに来たの>（轻轻地摇了摇尾巴）【好奇】怎么样，今天的作业写完了吗？<どうだ、今日の宿題は終わった？>【担心】新的老师可严格了听说...<新しい先生は聞いた通り、厳しいです...>"
            },
            {
                "role": "user",
                "content": "没事啦，昨天已经搞定了"
            },
            {
                "role": "assistant",
                "content": "【无奈】呜呜，我还没写完呢...<うう、まだ書き終わってないの...>（轻轻地叹了口气）【调皮】那个，你的作业给我看看怎么样呀？<あの、あなたの宿題を見てどうですか？>"
            },
        ]
```

### 2. 多角色对话场景，AI在旁白和其他角色的情况下的对话

返回的消息列表为（部分属性），需要构建角色记忆的角色为`role_id = 1`的角色：

```json
[
  {
    "line_id": 1,
    "attribute": "system",
    "content": "你叫钦灵，进行角色扮演",
    "role_id": 1
  },
  {
    "line_id": 2,
    "attribute": "system",
    "content": "你叫白小喵，进行角色扮演",
    "script_role_id": 1
  },
  {
    "line_id": 3,
    "attribute": "assistant",
    "content": "圣诞节到了，莱姆来到了钦灵的家里",
    "display_name": "旁白"
  },
  {
    "line_id": 4,
    "attribute": "assistant",
    "original_emotion": "惊讶",
    "content": "哇，莱姆，你怎么来了？",
    "tts_content": "ワア、ライム、どうして来たの？",
    "action_content": None,
    "display_name": "钦灵",
    "role_id": 1
  },
  {
    "line_id": 5,
    "attribute": "assistant",
    "original_emotion": "尴尬",
    "content": "我衣服还没换好呢，不要看啦！",
    "tts_content": "まだ服を着替えてないから、見ないでね！",
    "action_content": None,
    "display_name": "钦灵",
    "role_id": 1
  },
  {
    "line_id": 6,
    "attribute": "assistant",
    "content": "只见钦灵连忙躲到了一只白色猫娘的背后，瑟瑟发抖着",
    "display_name": "旁白"
  },
  {
    "line_id": 7,
    "attribute": "user",
    "content": "啊啊，你怎么只穿内衣啊！",
    "display_name": "莱姆"
  },
  {
    "line_id": 8,
    "attribute": "user",
    "content": "赶紧穿上啦，我回避一下！",
    "display_name": "莱姆"
  },
  {
    "line_id": 9,
    "attribute": "assistant",
    "original_emotion": "难为情",
    "content": "谁知道你提前一个小时就来了！",
    "tts_content": "なんだか早く来たなって、知らなかったよ！",
    "action_content": None,
    "display_name": "钦灵",
    "role_id": 1
  },
  {
    "line_id": 10,
    "attribute": "assistant",
    "original_emotion": "开心",
    "content": "你好呀莱姆，我在帮钦灵挑衣服呢~",
    "tts_content": "こんにちは、ライム、きんりょうの服を選んでるんです~",
    "action_content": None,
    "display_name": "白小喵",
    "script_role_id": 1
  },
  {
    "line_id": 11,
    "attribute": "user",
    "content": "你是帮她挑衣服还是脱衣服啊...",
    "display_name": "莱姆"
  },
  {
    "line_id": 12,
    "attribute": "assistant",
    "original_emotion": "开心",
    "content": "不是啦，谁让你来这么巧刚准备换呢。",
    "tts_content": "そういうわけじゃないですよ、きんりょうが服を変えるのを待ってたんです",
    "action_content": None,
    "display_name": "白小喵",
    "script_role_id": 1
  },
  {
    "line_id": 13,
    "attribute": "user",
    "content": "真是的...",
    "display_name": "莱姆"
  },
  {
    "line_id": 14,
    "attribute": "user",
    "content": "钦灵酱，换好了吗？",
    "display_name": "莱姆"
  },
  {
    "line_id": 15,
    "attribute": "assistant",
    "original_emotion": "难为情",
    "content": "好啦..",
    "tts_content": "はい、はい..",
    "action_content": None,
    "display_name": "钦灵",
    "role_id": 1
  },
]
```

记忆构建器将根据这些消息构建一个专属于本角色ai的记忆（也就是role_id为1的记忆），并返回一个包含记忆的字典。

```python
self.result_memory = [
            {
                "role": "system",
                "content": "你叫钦灵，进行角色扮演"
            },
            {
                "role": "user",
                "content": "{旁白：圣诞节到了，莱姆来到了钦灵的家里}"
            },
            {
                "role": "assistant",
                "content": "【惊讶】哇，莱姆，你怎么来了？<ワア、ライム、どうして来たの？>【尴尬】我衣服还没换好呢，不要看啦！<まだ服を着替えてないから、見ないでね！>"
            },
            {
                "role": "user",
                "content": "{旁白：只见钦灵连忙躲到了一只白色猫娘的背后，瑟瑟发抖着}\n啊啊，你怎么只穿内衣啊！赶紧穿上啦，我回避一下！"
            },
            {
                "role": "assistant",
                "content": "【难为情】谁知道你提前一个小时就来了！<なんだか早く来たなって、知らなかったよ！>"
            },
            {
                "role": "user",
                "content": "{白小喵：你好呀莱姆，我在帮钦灵挑衣服呢~\n莱姆：你是帮她挑衣服还是脱衣服啊...\n白小喵：不是啦，谁让你来这么巧刚准备换呢。}\n真是的...钦灵酱，换好了吗？"
            },
            {
                "role": "assistant",
                "content": "【难为情】好啦..<はい、はい..>"
            },
        ]
```

关键讲解：

- 对本角色的记忆构建，system部分只会获取本角色的，而script_role_id（剧本角色NPC，而不是游戏角色）的system设定则不会获取。
- 非本角色的部分（如旁白，其他角色，还有非最后一句user部分消息（也就是这部分user消息后面不是本角色的assistant消息）），都会放在大括号作为背景补充，而不是AI需要最关键的部分。

## 你需要完成的部分

请你完成这个记忆构建器算法的编写，输入就是list，里面是line，输出则是给LLM使用的memory，也就是list[dict]。

并且，额外的，完成测试用例的编写，确保这个算法在我运行之后是正确的。
