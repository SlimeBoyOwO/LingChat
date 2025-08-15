## 自定义人物指南

你是否希望自己的人物也闪耀光芒，在 LingChat 中陪伴？以下是一份基础的自定义人物指南，帮助你制作自己的人物并且上传到创意工坊！

### 0. 制作人物需要的材料

#### 为了制作自己的人物，你需要准备好他的基础设定

1. 立绘差分（19 张）和头像（一张）
2. 准备好为他的设定词
   > 没错，就这么点，很简单八！

### 1. 为人物创建文件夹结构

- LingChat 为人物的角色文件夹在 `game_data/characters/游戏角色` 中保存
- `characters` 文件夹内可以存放多个游戏角色，他们可以通过点击刷新人物列表或者重启软件导入
- 对于每个游戏角色，他们的文件夹应该如下所示:

```
诺一钦灵
|-avatars
|   |-高兴.png
|   |-伤心.png
|   |-...
|   |-头像.png
|-settings.txt
```

- 其中 `avatars` 文件夹用于存放 20 张图片，分别是 18 张对应不同情绪的差分，一张正常的待机表情，和一个用于展示的头像，都是 `png` 格式
- `settings.txt` 用于存放用于展示的相关信息。

### 2. 立绘准备（avatars 部分）

- avatars 准备部分非常简单，你只需要准备好包含所有以下文件名的图片文件即可（按照文件名排序，可用于对照）：

```
avatars
|-担心.png
|-调皮.png
|-尴尬.png
|-高兴.png
|-害怕.png
|-害羞.png
|-慌张.png
|-惊讶.png
|-难为情.png
|-情动.png
|-认真.png
|-伤心.png
|-生气.png
|-头像.png
|-无奈.png
|-兴奋.png
|-厌恶.png
|-疑惑.png
|-正常.png
|-自信.png
```

### 3. settings.txt 准备

核心文件 `settings.txt` 文件保存了一个角色所有的显示信息和性格等。以下是设定的详细说明：

```
title = 星空列车-音理            # 用于设定人物卡片标题
info = """                     # 多行，人物简介
火车，要出发了哦~
哥哥，准备好一起旅行了吗？
"""

ai_name = 音理                  # 设定对话框中，AI的名字
ai_subtitle = 邻家的女孩         # 设定对话框中，AI的子标题
user_name = 旅人                # 设定玩家的显示名字
user_subtitle = 列车の乘客       # 设定玩家的副标题
thinking_message = 音理的心脏为你跳动中.... # 设定思考提示文字

# 二选一部分，以下为可选项，看你选择的vits引擎
speaker_id = 4                 # 选择simple vits api的音色ID（如果使用simple vits api就加上这个）
model_name = neri              # 选择style bert vits2的模型名
tts_type = bv2                 #选择使用的tts类型(bv2,sgv,sbv,sva),不填默认sbv
language = ja                  #选择模型使用的语言，默认ja


scale = 1.9                    # 设定人物显示大小
offset = -3                    # 设定人物在Y轴上的显示便宜

bubble_top = 15                # 情绪气泡的显示Y轴位置
bubble_left = 23               # 情绪气泡显示的X轴位置

system_prompt = """            # 设定人物性格，多行
你是一个可爱的小女孩~
"""
```

> 其中，scale 和 offset 等可以通过在网页按 F12，选中对应的网页元素（如人物图片或者气泡位置），在 css 样式那设定 scale 和 offset 用于调试选择合适的数值。

### 4. 共享自己的人物到创意工坊

进入项目创意工坊的网站 -> [创意工坊](https://github.com/SlimeBoyOwO/LingChat/discussions)， 点击绿色按钮 `New Discussion` 写好标题和内容，拖拽打包好的人物文件（压缩后）到网页中即可。点击发布就可以啦。
