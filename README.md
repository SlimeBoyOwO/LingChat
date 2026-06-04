<div align="center">

# LingChat 
### 一款灵动の人工智能聊天陪伴助手

<img src="https://github.com/user-attachments/assets/ffccbe79-87ed-4dbc-8e60-f400efbbab26" width="800" style="border-radius: 12px; box-shadow: 0 4px 8px rgba(0,0,0,0.1);" alt="official_gif">

**选择你喜欢的人物，陪伴你度过每一个寂寞的夜晚。**

[📥 下载最新版本](https://github.com/SlimeBoyOwO/LingChat/releases) · [🐛 报告 Bug](https://github.com/SlimeBoyOwO/LingChat/issues) · [📖 源代码使用教程](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/%E6%BA%90%E4%BB%A3%E7%A0%81%E4%BD%BF%E7%94%A8.md)

</div>

---

## 🖥️ 快速指引 & 社区支持

<div align="center">
  <img width="400" style="border-radius: 8px; margin-right: 15px;" alt="preview_image" src="https://github.com/user-attachments/assets/a7db1801-a209-4948-abf2-7409b00e5017" />
</div>

<br>

| 📌 资源直达 | 🔗 链接 | 说明 |
| :--- | :--- | :--- |
| **安装互助群** | **QQ群：1055935861** | 纯安装问题请加此群，目前通过二维码进入。 |
| **代码报错求助** | [👉 帮助文档-代码报错](https://github.com/SlimeBoyOwO/LingChat/blob/main/README-help.md) | 遇到终端或后台代码红字报错时查看。 |
| **截图报错求助** | [👉 帮助文档-截图报错](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/Q&A.md) | 遇到弹窗或画面显示异常时查看。 |

---

## 🛠️ 核心功能列表

我们致力于提供最沉浸、最自由的 AI 陪伴体验。

| 🤖 AI 智能与情感 | 🎨 界面与自定义 |
| :--- | :--- |
| **🧠 永久记忆与独立存档**<br>每一个存档拥有独立的永久记忆，自由体验不同的对话风格。 | **🎭 完全自定义角色**<br>支持导入自己的 OC 或喜欢的游戏人物，开启专属对话。 |
| **💓 自研情绪识别模型**<br>自动判定 AI 每次对话的情绪，告别机械式回复。 | **👗 服装切换与互动**<br>支持切换角色服装，更加入了摸摸角色等趣味互动功能！ |
| **👁️ 视觉感受与主动窥屏**<br>自动识别你的桌面状态（工作/游戏/挂机），AI 会主动关心你。 | **✨ 动态UI与沉浸式体验**<br>表情、动作、气泡随 AI 情绪改变，搭配不同背景与音乐聊天。 |

| 🎙️ 语音与剧情扩展 | ⚙️ 实用工具与兼容性 |
| :--- | :--- |
| **🎧 真实语音与音效陪伴**<br>支持接入 Vits 语音服务或对话音效，用真实的耳语调动你的真心。 | **🍅 内置效率工具**<br>自带番茄钟、日程、待办清单，AI 会根据你的状态贴心提醒。 |
| **📜 多角色剧本与羁绊系统**<br>支持导入剧本进行多角色对话，为角色编写羁绊剧情，解锁成就！ | **💻 极致的系统兼容**<br>兼容 Linux/macOS，且完美兼容 32 位 Windows 7 及老旧 CPU！ |

---

## 🚀 快速上手指南

### 🔧 准备工作
> [!NOTE]
> **API 申请：** 在 [DeepSeek 官方网站](https://platform.platform.com/) 或其他大模型平台申请属于你的 API 密钥，并确保账户内有余额。

### 📦 第一步：下载与安装
1. 根据你的系统选择合适的版本：
   - 💻 **Win10 64位及以上**：请下载最新版 `0.4.0 Pre`。
   - 💻 **32位或老旧机器**：请前往 [Issues #379](https://github.com/SlimeBoyOwO/LingChat/issues/379) 下载兼容版（如 `LingChat v0.4.0-pre Python3.8 win32.7z`）。
2. 在 [Releases](https://github.com/SlimeBoyOwO/LingChat/releases) 页面下载 `LingChat_setup.exe` 或 `.7z` 压缩包。
3. 安装或解压后，双击 `LingChat.exe` 或 `启动器.bat` 运行。

> [!WARNING]  
> 解压后如果发现 `LingChat.exe` 不见了，通常是被 Windows Defender 误杀了。请进入 **Windows 安全中心 -> 病毒和威胁防护** 允许该文件运行。

### ⚙️ 第二步：首次启动配置
1. 启动程序后，点击**开始游戏**。
2. 打开右上角菜单，进入**【高级设置】**。
3. 填入必填信息：**大模型类型**、**API Key** 及 **模型信息**。
4. 滑动到底部保存配置，**完全关闭程序及黑色控制台窗口**，重新启动即可使用！

> [!IMPORTANT]
> - **加载卡住：** 若 `LingChat.exe` 无限卡加载页，请使用谷歌等现代浏览器访问 `localhost:8765` 进入。
> - **重启注意：** 重启初始化前，务必确保前端、后端、网页及 cmd 窗口已全部关闭，否则可能导致角色消失。
> - **色彩灰暗：** 支持 P3 色域的屏幕若出现画面发灰，请尝试更换正常颜色显示的浏览器进入。
> - **老电脑注意：** 老旧 CPU（如 Intel Atom Z540）启动时间可能长达 7~9 分钟，请耐心等待 ☕。

---

## 🧩 扩展功能配置

<details>
<summary><b>🎧 配置 Vits 语音功能（点击展开）</b></summary><br>

1. 启动游戏后，在菜单【文字】部分找到语音引擎的下载链接，下载后缀为 `.7z` 的对应版本。
2. 解压后，双击 `..01 启动API服务.bat` 即可开始使用。
3. **性能提示**：如果你是核显或老电脑，单句语音生成可能需要 1 分钟；拥有 GPU 的设备可在 1 秒内生成。核显用户若遇大量报错可能无法使用此功能。
4. **自定义角色使用**：若想给自定义角色使用语音，需在 `game_data/characters/<角色名>/settings.txt` 中修改 `model_name` 参数为导入模型的名称。
</details>

<details>
<summary><b>👁️ 配置视觉大模型功能（点击展开）</b></summary><br>

1. 在通义千问等平台获取视觉模型 API 👉 [阿里云视觉模型 API 获取](https://bailian.console.aliyun.com/?tab=api#/api)
   *(注：阿里云 API 默认赠送额度，且对于本项目绝对够用)*
2. 在主页进入【日程】->【主动对话】。
3. 填入 `VD_API_KEY`，如需更换其他模型可填写对应的 `base_url` 和 `model_name`。
4. 保存后重启软件。
5. **如何触发**：在聊天中发送 `“看桌面”` 或 `“看看我的桌面”`。配置主动对话后，AI 将拥有主动观察你在干什么的能力（*注意不要在玩黄油时被抓包哦 _^▽^_*）。
</details>

<details>
<summary><b>🧪 加入最新版测试与开发（点击展开）</b></summary><br>

- 我们会随时将更新推送到 [`develop` 分支](https://github.com/SlimeBoyOwO/LingChat/tree/develop)，并在 [Issues](https://github.com/SlimeBoyOwO/LingChat/issues) 发布开发日志。
- 你可以参考 [源代码使用教程](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/%E6%BA%90%E4%BB%A3%E7%A0%81%E4%BD%BF%E7%94%A8.md) 随时获取最新的开发版更新。
- *注：开发版可能不稳定，欢迎随时提交 Bug 反馈！*
</details>

---

## 🔗 致谢与开源灵感

本项目的实现离不开这些优秀开源作品的先驱者，送上由衷的致谢 🌼：

- **[Zcchat](https://github.com/Zao-chen/ZcChat)**：本项目的灵感来源，提供了优秀的 `Vits` 模型和人物素材。（请去给他们点个 Star 吧 ❤）
- **[Simple-Vits-API](https://github.com/Artrajz/vits-simple-api)**：实现了简单易用的 VITS 语音合成 API。
- **[Style-Bert-VITS2](https://github.com/litagin02/Style-Bert-VITS2)**：实现了 Bert-VITS 的语音合成与训练，极少数据量也能达到完美效果！
- **[ProgrammingVTuberLogos](https://github.com/Aikoyori/ProgrammingVTuberLogos)**：提供了超可爱的标题风格灵感。
- **[Emotion Training](https://github.com/SlimeBoyOwO/Emotion-Model-Trainer)**：用于实现 18 种短句情绪识别的人工智能模型训练。

---

## 🌸 写在最后

- 本项目为了快速开发使用了一些 AI 辅助工具，如有不足欢迎在 Issues 中指出！
- 感谢一路结识的开发者们，大家都是**香软可爱**又厉害的大佬们~ 如果你有开发意向，欢迎联系我（开发者群号藏在 GitHub 中 ❤）。
- 这个项目最初是一个超小型学习项目，现在虽然变得庞大并应用了更多的软件工程架构思想，但依然非常欢迎大家以此作为学习的参考 (qwq)。

<details>
<summary><b>⚠️ 免责声明与素材版权说明</b></summary>

> - **素材来源：** 气泡与音效素材、初始界面来源于《碧蓝档案》；对话哔哔音效来源于《Undertale》，**请勿商用**。
> - **立绘版权：** 默认人物立绘由开发者本人绘制，请勿乱用、商用或用于奇怪的地方。
> - **责任声明：** 请对 AI 生成的内容及您的使用行为负责，不要肆意传播不良信息。
> - **数据调查：** 我们秉持完全自愿原则，您可以在 [Ling Chat 硬件调查](https://dash.myhblog.dpdns.org/) 中查看匿名调查数据。
</details>

---

<div align="center">

### ⭐️ 喜欢这个项目吗？请给我们点一个 Star！
**这是我们提升影响力和持续维护的最大动力！**

[![Star History Chart](https://api.star-history.com/svg?repos=SlimeBoyOwO/LingChat&type=Date)](https://www.star-history.com/#SlimeBoyOwO/LingChat&Date)

*© LingChat 制作团队*
</div>
