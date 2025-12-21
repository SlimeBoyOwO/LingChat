---
title: LingChat dev版本使用文档
description: 一份基础的LingChat dev使用文档，教你如何使用 LingChat 的 dev 版本
outline:
  level: [2, 3]
---

# 欢迎你，勇敢的开发者

## 前言：

- 既然你打开这个文档，说明你已经准备吃 develop 的石山代码了。以下是你要先了解的信息：

1. develop 版本非常 **不稳定** ，是开发版本， **不保证运行的健壮性**
2. develop 版本 **开发比较激进** ，经常留下 bug 或者屎山代码，项目团队不接受无脑的指点， **一切不以代码为基础的指责不予受理**
3. develop 版本有 **最新的大胆的想法，和最优秀的，优化后性能和特性** （确信）
4. 这个版本比较 **适合想要学习代码，或者想参与开发的人，不适合普通用户** ！

> [!NOTE]
> 本教程以 Python UV + Nodejs pnpm + Sbv2 TTS 为主，有经验者自行选择 conda/mamba 或者其他包管理器，按需处理。
>
> 如果你在中国大陆，任何时候都不要忘记给工具配置镜像，具体各工具镜像可问ai

## 安装教程:

### Step 1: 克隆项目到你的电脑

1. 打开 Win+R 输入 cmd 回车，进入 cmd 窗口，更改目录到你的想要安装的位置，比如你想在 D 盘的这个位置安装的话

```
D:
cd D:\NoiProjects\GitHub\temp
```

2. 然后，使用下面的命令克隆（需要有 Git 环境，不会问 AI）

方式一:
```
git clone https://github.com/SlimeBoyOwO/LingChat.git
cd LingChat
git checkout develop
```
方式二:
```
git clone -b develop https://github.com/SlimeBoyOwO/LingChat.git
```

3. 这样，你就获取了 LingChat 的源码仓库，并且切换到了 develop 开发分支

### Step 2: Python 配置

1. 安装 Python 找官网下载就行（版本最好是 3.12~3.13） ，这都不会你还是去玩 release 吧（

2. 首先，你需要下载一个 uv 库，这是现代的 Python 管理库

```
pip install uv
```

3. 接着，创建虚拟环境，并且安装所需的依赖

```
uv venv --python 3.13
.venv\Scripts\activate
uv pip install .
```

4. 恭喜你，你已经正确配置好了 Python 环境

### Step 3: Vite+Vue 配置

1. 这一部分很简单，首先你需要有 nodejs 环境，直接下载就行，温馨提示最后一个安装解密有个下载 cholately 什么的，不要勾选，不然你的 python 环境很有可能爆炸，记得勾选 ADD PATH 选项
2. 接着，打开 cmd 进入项目根目录下的 `frontend_vue`文件夹，输入以下指令：

```
npm install -g pnpm
pnpm i
```

3. 等待执行完毕即可

### Step 4: 项目基础配置

1. 找到项目根目录的 `.env.example` 文件，复制一份并且重命名为 `.env`
2. 填写基础的信息，包括 API_KEY 等，具体和 `README.md` 差不多，不填也行
3. 打开 `start.bat` 启动程序，他会打开两个窗口，一个是前端，一个是后端，注意区分。后端有 LingChat 的 LOGO 提示
4. 如果你看到下面的信息，基本上就没问题了：

```
[INFO]: Application startup complete.
[INFO]: Uvicorn running on http://0.0.0.0:8765 (Press CTRL+C to quit)
```

其中也包含了这样的提示：

```
[ERROR]: × 情绪分类模型加载异常 - 加载情绪分类模型失败: Error no file named pytorch_model.bin, model.safetensors, tf_model.h5, model.ckpt.index or flax_model.msgpack found in directory D:\NoiProjects\GitHub\temp\LingChat\ling_chat\third_party\emotion_model_18emo.
```

5. LingChat 使用了自训练的情感分类模型，由于模型较大，所以没有放在 Github 上，需要的话去开发群下载最新的情感分类模型（25/11/1: 目前刚刚换成 onnx 模型，暂未公开，只能在群里下载）放在上面提示的目录中：

```
D:\NoiProjects\GitHub\temp\LingChat\ling_chat\third_party\emotion_model_18emo
```

6. 完成下载和目录放置后，Ctrl+C 可以终止后端程序运行，然后再输入 n 就可以重新启动后端了

### Step 5: 正式运行项目

1. 浏览器输入 `localhost:5173` 可以进入前端页面。打开右上角的高级设置，里面可以配置各种 api 信息还有个性化的开发者选项。点击保存后重启后端程序（Ctrl+C, N）就可以生效了。

> [!IMPORTANT]
> 没事别特喵碰实验性功能的开关，倒时候整个软件炸了哭了就不关我事了吼

2. 一切配置完毕后，输入对话，开始聊天吧！

### Step 6: 语音安装教程

> [!NOTE]
> 语音部分详细介绍请看 [此文档](../manual/expand/voice.md)，人物卡制作详见 [此文档](character_guide.md)

1. 前往网站 [SBV2](https://github.com/litagin02/Style-Bert-VITS2)
2. 点进 release，下载最新的 release 文件
3. 解压，然后点击 install.bat （不是 CPU 的那个，当然没有显卡或仅推理可用 CPU 版本），记得选一个好的项目目录（路径全英文），不然后面可能不方便迁移
4. 如果安装过程有问题，比如出现某个依赖包失败，要自行修改 `pyproject.toml` 内相关依赖包的版本（一般不用）
5. 安装完毕后会自动弹出一个浏览器窗口，表示安装完毕
6. 之后需要添加 sbv2 模型，只需要在 model_assests 里面添加即可
7. 之后，只需要打开该目录的 server.bat 就可以为 Lingchat 启动语音服务了
8. 在 LingChat 中，修改某个角色使用 sbv2 的方法是：

```
# ling_chat\data\game_data\characters\诺一钦灵\settings.txt

voice_models = {
    "sva_speaker_id": "0",
    "sbv2_name": "这里填写模型名称",
    "sbv2_speaker_id": "0",
    "bv2_speaker_id": "0",
    "sbv2api_name": "",
    "sbv2api_speaker_id": "0",
    "gsv_voice_text": "",
    "gsv_voice_filename": "",
    "gsv_gpt_model_name": "",
    "gsv_sovits_model_name": "",
    "aivis_model_uuid": ""
}
tts_type = "sbv2"
```

> [!IMPORTANT]
> 后端出现未启用 TTS 的信息，要重启后端

## 其它

如果觉得看完还 **意犹未尽** 或是觉得本教程 **太拉了**，你可以看[这一篇](windows_dev.md) ~~或者自己写一篇投稿~~
