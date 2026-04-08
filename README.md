# 🐈✨ LingChat - 灵动の人工智能聊天陪伴助手

![official](https://github.com/user-attachments/assets/ffccbe79-87ed-4dbc-8e60-f400efbbab26)

## 🖥️ 支持操作系统：

- Windows、Linux/Macos 均可运行。Linux/Macos 用户请查看额外的使用说明。 
- 社区安装 QQ 互助群：1055935861【纯安装问题请加入此群，不要去开发者群.jpg，目前好像只能二维码进入了】
- 安装问题解答文档[用户帮助文档-代码报错](https://github.com/SlimeBoyOwO/LingChat/blob/main/README-help.md), [用户帮助文档-截图报错](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/Q&A.md)

<img width="408" height="563" alt="image" src="https://github.com/user-attachments/assets/a7db1801-a209-4948-abf2-7409b00e5017" />

## 🛠 功能列表

- [x] 选择你喜欢的人物，陪伴你聊天度过寂寞的夜晚
- [x] 内嵌存档+永久记忆功能，每一个存档拥有独立的永久记忆，可以体验不同的对话风格
- [x] 使用自训练的 AI 情绪识别模型，自动判定 AI 的每次对话情绪
- [x] 表情，动作，气泡随着 AI 的情绪改变，提供灵活的 AI 聊天体验
- [x] 搭配 Vits 语音服务或对话音效，用真实的耳语调动你的真心
- [x] 支持自定义角色，可以用自己的 oc 或者游戏人物与自己对话
- [x] 清爽的设置菜单，高度自定义的设置选项，可搭配不同背景和音乐聊天
- [x] 支持视觉感受，主动窥屏功能，自动识别用户的状态是工作还是游戏还是挂机
- [x] 程序内自带番茄钟，日程，待办清单功能，并且支持让 ai 根据你的状态提醒
- [x] 支持切换角色服装，摸摸角色等功能！
- [x] 支持为角色编写羁绊剧情，解锁更多剧情，成就！
- [x] 支持导入剧本，进行多角色对话，自定义剧情，场景！
- [x] 兼容32位Windows系统（Windows 7及以上）！ 也兼容老古董CPU尝鲜！

## ⭐ 快速上手

### Step -1: 选择适合的版本

- 如果是Windows10 64位或者更高请使用最新版本`0.4.0 Pre`！
- 如果是32位机器请用兼容版喵~

### Step 0: 开始之前的准备

- 在 DeepSeek 或者其他大模型网站中，申请自己的 API 密钥，并且保证有余额供使用 -> [DeepSeek 的官方 API 获取网站](https://platform.deepseek.com/)

### Step 1: 下载软件

- 在[release](https://github.com/SlimeBoyOwO/LingChat/releases)中，找到最新的版本，下载如 `LingChat_setup.exe` 的文件，下载完成后运行并安装 LingChat。
- 点击桌面快捷方式或安装目录下的 `LingChat.exe`或者`启动器.bat`启动程序
- 您也可以使用如 LingChat.x.x.x.7z 的文件解压后使用
- 补充：老机器在[issues](https://github.com/SlimeBoyOwO/LingChat/issues/379)里面寻找适合自己的版本，下载如`LingChat v0.4.0-pre Python3.8 win32.7z`的文件，运行方式如上

#### 温馨提示：

> 解压完成后可能会发生 `LingChat.exe` 不见了的情况，这多半是由于 Windows Defender 把它当病毒干掉了。需要手动打开**Windows 安全中心**，选择**病毒和威胁防护**一栏，允许该威胁。

### Step 2: 首次启动配置

- 启动程序后，直接点击开始游戏，点开右上角的菜单，点击【高级设置】，输入自己选用的大模型类型和 API，模型信息等（**这些是必填信息**）
- 设置完毕后，滑动到最下方，点击保存配置。关闭黑不溜秋的窗口和 LingChat 程序，重新启动程序，就可以使用啦！
  > [!IMPORTANT]
  >
  > 1. **有些用户的电脑启动`LingChat.exe`之后会无限卡在加载页，请在现代浏览器如谷歌中输入`localhost:8765`进入程序**
  > 2. **当你关闭程序准备重启初始化时候，务必保证前端和后端都关闭（exe 或者浏览器的网页，还有 cmd 窗口），否则可能出现进去人物消失的情况**
  > 3. **对于支持 P3 色域显示的电脑，如果你感觉进去画面灰蒙蒙的，可以切换正常显示颜色的浏览器输入地址进入**
  > 4. **对于过于老旧、运行缓慢的CPU们，启动时间可能会大幅延长（如使用Intel Atom Z540启动一次的时间为7~9分钟），只能稍微忍忍了TAT**

### Step 3：语音功能使用（从这里开始的以下步骤属于扩展功能，按需进行）

- 开始游戏后点击右上角的菜单，在【文字】部分下面有语音引擎的下载链接，根据自己电脑的情况下载对应的版本（后缀为.7z的文件）
- 解压，点开`..01 启动API服务.bat`就可以直接用了！
  > 1. 经过反映，如果你的电脑是核显或者太久以前的电脑，单个语音可能要一分钟才能生成，而 GPU 可以 1 秒内生成，而且会有大量报错可能，核显用户大可能只能放弃语音功能了（哭哭）

#### 温馨提示：

> 要是想自定义角色使用这个功能，需要在 `game_data/characters/<角色名>/settings.txt` 中设定 `model_name` 的参数为导入的模型的名字  

### Step 4：视觉模型功能使用

- 从通义千问或者其他拥有视觉感知的大模型网站中，获取 API -> [阿里云的相关视觉模型 API 获取网站](https://bailian.console.aliyun.com/?tab=api#/api)
- 点开主页面的日程，点击主动对话，里面可以填写 `VD_API_KEY`，填写完毕后划到下面点击保存后重启软件即可使用。
- 如果你有更换其他模型的需求，填写不同的`base_url`和`model_name`即可
- 阿里云 API 默认赠送额度，不需要充值（不过阿里云好像百万token有使用时间限制）， _而且对于这个项目肯定够用一辈子了_ 。

#### 温馨提示：

> 设定完毕后，可以通过在与 AI 对话的对话中，包含 `“看桌面”` 或者 `“看看我的桌面”` 来触发视觉感知，允许 AI 观察你的屏幕并做出回应
> 主动对话中，配置视觉模块后，AI 将拥有主动看你在干什么的能力，小心不要玩黄油？(*^▽^*)

### Step 6: 加入最新版的测试

- 我们一直在更新 LingChat，所有更新都会随时推送到[develop](https://github.com/SlimeBoyOwO/LingChat/tree/develop)中，我们也会在[issues](https://github.com/SlimeBoyOwO/LingChat/issues)中发布开发日志。
- 你可以参考[源代码使用教程](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/%E6%BA%90%E4%BB%A3%E7%A0%81%E4%BD%BF%E7%94%A8.md)来使用 LingChat 的源代码，并随时获取最新的 develop 开发版更新。
- 开发版是不稳定的版本，如果遇到任何 Bug，欢迎向我们反馈！

## 🔗 相关 & 致谢链接

- [Zcchat](https://github.com/Zao-chen/ZcChat): 本项目的灵感来源，可以在这里找到 `Vits` 模型和人物素材。可以的话也帮他们点个 stars 吧 ❤
- [Simple-Vits-API](https://github.com/Artrajz/vits-simple-api): 该项目实现了基于 `VITS` 的简单语音合成 API。如果你不是核显建议下载 GPU 版本，速度快。核显就用 CPU。
- [Style-Bert-VITS2](https://github.com/litagin02/Style-Bert-VITS2)：该项目实现了 `Bert-VITS` 的语音合成和训练，你可以用这个进行语音训练和推理，少量数据量就可以达到很棒效果！
- [ProgrammingVTuberLogos](https://github.com/Aikoyori/ProgrammingVTuberLogos)：LingChat 的标题风格，可爱滴捏，画风参考这个项目~
- [Emotion Training](https://github.com/SlimeBoyOwO/Emotion-Model-Trainer): 人工智能模型训练，用于实现 18 种短句情绪识别
- 本项目的实现离不开这些优秀开源作品的先驱者，在这里我们送上由衷的致谢 🌼

## 🌸 一些小话

- 本项目为了快速开发用了很多 AI 工具，有做的不好的地方欢迎指出！我们欢迎各位开发者或用户提出 issues！
- 感谢一路结识的开发者，都是 **香软可爱** 又厉害的大佬们~ 如果你有开发意向可以联系我！开发者群号就藏在 GitHub 中 ❤
- 本项目更多作为一个超小型的学习项目，由于文件结构非常简单， ~~欢迎有兴趣的人学习~~ 。现在变大了，应用了很多软件工程的架构思想，也欢迎学习啦 qwq

## 🔍 其他

> 本项目使用的气泡+音效素材+初始界面来源于碧蓝档案，其中对话哔哔音效来源于 Undertale，请勿商用  
> 默认人物立绘作者为我本人，请勿乱用（商用和奇奇怪怪的东东（bushi））
> 请对 AI 生成的东西和使用负责，不要肆意传播不良信息
> 有其他问题可以提 issues 捏~
> 关于硬件调查，我们完全秉持自愿原则，您可以在[Ling Chat 硬件调查](https://dash.myhblog.dpdns.org/)中查看调查数据

## ⭐️ 星星（觉得好的话务必点一个 Star，这是我们提升影响力和持续做下去的动力！）

[![Star History Chart](https://api.star-history.com/svg?repos=SlimeBoyOwO/LingChat&type=Date)](https://www.star-history.com/#SlimeBoyOwO/LingChat&Date)

© LingChat 制作团队
