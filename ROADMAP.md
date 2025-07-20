# v0.4.0 版本更新前瞻

![b3377564da7e05ecf2600aaae896db66](https://github.com/user-attachments/assets/25dac3ec-7cf7-4675-819c-a055e4b3b928)

# 一、LingChat 0.4 要做什么？

在LingChat 0.1，我们完成了一个使用仿碧蓝档案UI的，有语音和立绘切换的，和AI聊天的系统。

在LingChat 0.2，我们给系统添加了RAG实现跨越对话的永久记忆，时间感知，以及存档读档功能

在LingChat 0.3，我们完善了各个功能，添加了多人物支持，创意工坊，音乐和背景导入，使得LingChat 可以被深度自定义，并增加了易用性。

至此，一个“使用仿碧蓝档案UI的，有语音和立绘切换的，和AI聊天的系统”已经完善，是时候做一些前人没有做过的东西了。



在LingChat 0.4，我们将会为LingChat添加以下功能。

- **长线剧情系统**：支持使用类似galgame的剧情预设，直接兼容传统galgame的预设剧本和分歧选择，同时支持将一部分甚至全部的剧情**由AI驱动**。你将在剧情内日常的场合，停下来和主角自由的谈心，聊够了在继续剧情；你将不再局限于点击选项来选择剧情分歧，而是真正进入故事，说出你想说的话，影响主角做出重要选择，或者**劝说**主角真正的回心转意。
- **剧本杀/跑团模式**：支持使用类似剧本杀/跑团模式的剧情预设，由一个DM（主持人）来掌控剧情的发展，你将体验到诸如随着时间的推移获得越来越多的信息，判断“谁是凶手”，等类剧情游戏
- **随机事件生成器**：轻量化的剧情引导，如你和你的oc探索地下迷宫的过程中，由LLM生成你们下一个房间的见闻
- **大量的预设小游戏**：和你的一个甚至多个oc人设玩一把狼人杀，真心话大冒险，甚至恶魔轮盘赌等经典互动游戏

这些功能互不冲突需要一个从头开始精心设计的框架，所以这些功能不会逐次更新，而是在不停排查兼容性调优后，设计一个兼容这些所有模式的核心逻辑，讲这些功能**一次性全部实现**。

我们还会编写可视化剧情编辑系统，编写详尽的说明文件，让所有人可以编写出属于自己的，可以参与其中的故事——LingChat 0.4，更接近于一个“剧情编辑器”+“剧情渲染器”，提供一个AI时代的剧情展现平台。

# 二、其他零碎的工作计划

## 帮助文档编写

- 编写详细的帮助文档，包括小白教程和开发须知，使用超链接在 Readme 引用
  - [Linux 部署教程](others/document/Linux部署教程.md)
  - [Release 部署教程](others/document/Release部署教程.md)
  - [开发文档](others/document/开发文档.md)

## 长剧情功能更新

- **场景 CG 联动**：添加将场景 CG 以 prompt 发送到 llm 的功能，以及随着剧情推进改变场景 CG 的功能。
  - 实现 CG 影响对话，如，在场景为卧室 CG 时向 llm 发送：“现在你们在卧室里，时间是晚上”。此外，要给予 AI 自行改变场景的能力，如随剧情推进，场景从卧室切换为学校，或者同场景傍晚切换到黑夜。**此功能为长线剧情更新的必要前置**
  - 注意，由于已经有了发送系统时间功能，配置文件中应提供选项，使用系统时间，使用 CG 时间，不发送时间；此外，考虑增加随系统时间切换 CG（白天 → 黄昏 → 夜晚）功能。
- **大地图功能**：考虑添加大地图功能，例如，点击“家”就回家，点击“学校”就去学校，同时影响 CG 和对话，可能还会影响剧情。这里可以参考夏日口袋的大地图与长线剧情线设计。

## 陪伴系功能更新

- **桌宠启动模式**✅：开发轻量化的启动模式，只有人物，无边框，可选方形背景或背景透明，会显示在桌面顶层
- **添加视觉能力**✅：添加多模态支持，让 AI 拥有桌面视野，后续基于多模态视觉开发各种陪伴功能。若 DeepSeek 迟迟不更新原生多模态，可以考虑使用 Qwen

## 系统底层设计与优化

- **llm 接口重构✅**：重构 deepseek.py，分别兼容以下接口：
  - DeepSeek，Qwen，GPT，Claude，Gemini 官方接口
  - 支持多模态传输的标准格式第三方 OpenAI 请求
  - 标准 Ollama，LM Studio 本地部署接口
  - 未来可不断扩展新增的接口格式
- **手机适配**：重构前端界面，适配手机 UI，未来可能登录官方服务器使用；若有安卓开发成员，可以考虑开发轻量化的手机版本
- **语音接口**：添加 GPTSoVIS 以及其他主流语音实现的支持
- **网络接口**：添加可以在网络获取信息的接口，可能可以实现：
  - 官方团队维护和发布的虚拟世界突发事件，让 LingChat 宇宙的事件演进
  - 日报功能，通过可靠的稳定的信源，再完善一套好用的 Prompt，以角色的性格和口吻播报当日新闻/本周新闻
  - 可能的后续其他功能
- **角色切换**✅：增加 AI 角色切换功能，增加 AI 名称修改
- **设置窗口**✅：用可视化的设置 UI 代替手动修改.env
- **启动速度优化**：取消重量级库的依赖，优化启动速递
- **永久记忆更新**：添加可以一建清除 RAG 缓存的.bat
- **启动动画添加✅**：在启动时添加日志动画，以点缀跳过漫长的启动等待
- **依赖去除✅**：本地化 RAG，取消必须连接抱脸的限制
- **降低延迟**：将语音改为流式传输，减少等待时间

## 服务器与团队规划

- **新成员招募**：在新视频发布招募公告
- **服务器维护团队**
  - 维护 LingChat 项目的官方服务器
  - 维护和发布的虚拟世界突发事件，让 LingChat 宇宙的事件演进
- **AI 炼丹师**
  - 制作/训练新人物的 vits 语音
  - 使用 AI 绘制大量的场景 CG 和人物立绘
  - 制作符合 LingChat 画风的 CG 和人物 lora，搭建 ComfyUI 工作流，通过 controlnet 实现通过输入人物提示词就直接生成 18 个差分表情，以支持用户的立绘自定义，直接导入本项目（tip：这确实是可以实现的）
- **编剧**
  - 编写长线剧情和新人设
- **画手**
  - 需要对大量使用 AI 工具有包容心——大部分画手不能容忍 AI 工具

# Collaborator

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section --> <!-- prettier-ignore-start --> <!-- markdownlint-disable --> <table> <tbody> <tr> <td align="center" valign="top" width="25%"> <a href="https://github.com/SlimeBoyOwO"><img src="https://avatars.githubusercontent.com/SlimeBoyOwO?s=100" width="100px;" alt="SlimeBoyOwO"/><br /><sub><b>钦灵</b></sub></a><br /> </td> <td align="center" valign="top" width="25%"> <a href="https://github.com/T-Auto"><img src="https://avatars.githubusercontent.com/T-Auto?s=100" width="100px;" alt="T-Auto"/><br /><sub><b>风雪</b></sub></a><br /> </td> <td align="center" valign="top" width="25%"> <a href="https://github.com/Vickko"><img src="https://avatars.githubusercontent.com/Vickko?s=100" width="100px;" alt="Vickko"/><br /><sub><b>vickko</b></sub></a><br /> </td> <td align="center" valign="top" width="25%"> <a href="https://github.com/0x00-pl"><img src="https://avatars.githubusercontent.com/0x00-pl?s=100" width="100px;" alt="0x00‑pl"/><br /><sub><b>PL</b></sub></a><br /> </td> </tr> </tbody> </table>
