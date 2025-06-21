# v0.3.0版本更新前瞻

![b3377564da7e05ecf2600aaae896db66](https://github.com/user-attachments/assets/25dac3ec-7cf7-4675-819c-a055e4b3b928)

看什么看，不知道github可以发动图吗

## 帮助文档编写

- 编写详细的帮助文档，包括小白教程和开发须知，使用超链接在Readme引用
  -  [Linux部署教程](others/document/Linux部署教程.md) 
  -  [Release部署教程](others/document/Release部署教程.md) 
  -  [开发文档](others/document/开发文档.md) 

## 长剧情功能更新

- **场景CG联动**：添加将场景CG以prompt发送到llm的功能，以及随着剧情推进改变场景CG的功能。
  - 实现CG影响对话，如，在场景为卧室CG时向llm发送：“现在你们在卧室里，时间是晚上”。此外，要给予AI自行改变场景的能力，如随剧情推进，场景从卧室切换为学校，或者同场景傍晚切换到黑夜。**此功能为长线剧情更新的必要前置**
  - 注意，由于已经有了发送系统时间功能，配置文件中应提供选项，使用系统时间，使用CG时间，不发送时间；此外，考虑增加随系统时间切换CG（白天→黄昏→夜晚）功能。
- **大地图功能**：考虑添加大地图功能，例如，点击“家”就回家，点击“学校”就去学校，同时影响CG和对话，可能还会影响剧情。这里可以参考夏日口袋的大地图与长线剧情线设计。
- **长线剧情系统**：可以参考安科文学和跑团的长线剧情AI应用与实现


## 陪伴系功能更新

- **桌宠启动模式**✅：开发轻量化的启动模式，只有人物，无边框，可选方形背景或背景透明，会显示在桌面顶层
- **添加视觉能力**✅：添加多模态支持，让AI拥有桌面视野，后续基于多模态视觉开发各种陪伴功能。若DeepSeek迟迟不更新原生多模态，可以考虑使用Qwen




## 系统底层设计与优化

- **llm接口重构**：重构deepseek.py，分别兼容以下接口：
  - DeepSeek，Qwen，GPT，Claude，Gemini官方接口
  - 支持多模态传输的标准格式第三方OpenAI请求
  - 标准Ollama，LM Studio本地部署接口
  - 未来可不断扩展新增的接口格式
- **手机适配**：重构前端界面，适配手机UI，未来可能登录官方服务器使用；若有安卓开发成员，可以考虑开发轻量化的手机版本
- **语音接口**：添加GPTSoVIS以及其他主流语音实现的支持
- **网络接口**：添加可以在网络获取信息的接口，可能可以实现：
  - 官方团队维护和发布的虚拟世界突发事件，让LingChat宇宙的事件演进
  - 日报功能，通过可靠的稳定的信源，再完善一套好用的Prompt，以角色的性格和口吻播报当日新闻/本周新闻
  - 可能的后续其他功能
- **角色切换**✅：增加 AI 角色切换功能，增加 AI 名称修改
- **设置窗口**✅：用可视化的设置UI代替手动修改.env
- **启动速度优化**：取消重量级库的依赖，优化启动速递
- **永久记忆更新**：添加可以意见清除RAG缓存的.bat
- **启动动画添加**：在启动时添加日志动画，以点缀跳过漫长的启动等待
- **依赖去除**：本地化RAG，取消必须连接抱脸的限制
- **降低延迟**：将语音改为流式传输，减少等待时间


## 服务器与团队规划

- **新成员招募**：在新视频发布招募公告
- **服务器维护团队**
  - 维护LingChat项目的官方服务器
  - 维护和发布的虚拟世界突发事件，让LingChat宇宙的事件演进
- **AI炼丹师**
  - 制作/训练新人物的vits语音
  - 使用AI绘制大量的场景CG和人物立绘
  - 制作符合LingChat画风的CG和人物lora，搭建ComfyUI工作流，通过controlnet实现通过输入人物提示词就直接生成18个差分表情，以支持用户的立绘自定义，直接导入本项目（tip：这确实是可以实现的）
- **编剧**
  - 编写长线剧情和新人设
- **画手**
  - 需要对大量使用AI工具有包容心——大部分画手不能容忍AI工具

# Collaborator
<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section --> <!-- prettier-ignore-start --> <!-- markdownlint-disable --> <table> <tbody> <tr> <td align="center" valign="top" width="25%"> <a href="https://github.com/SlimeBoyOwO"><img src="https://avatars.githubusercontent.com/SlimeBoyOwO?s=100" width="100px;" alt="SlimeBoyOwO"/><br /><sub><b>钦灵</b></sub></a><br /> </td> <td align="center" valign="top" width="25%"> <a href="https://github.com/T-Auto"><img src="https://avatars.githubusercontent.com/T-Auto?s=100" width="100px;" alt="T-Auto"/><br /><sub><b>风雪</b></sub></a><br /> </td> <td align="center" valign="top" width="25%"> <a href="https://github.com/Vickko"><img src="https://avatars.githubusercontent.com/Vickko?s=100" width="100px;" alt="Vickko"/><br /><sub><b>vickko</b></sub></a><br /> </td> <td align="center" valign="top" width="25%"> <a href="https://github.com/0x00-pl"><img src="https://avatars.githubusercontent.com/0x00-pl?s=100" width="100px;" alt="0x00‑pl"/><br /><sub><b>PL</b></sub></a><br /> </td> </tr> </tbody> </table>
