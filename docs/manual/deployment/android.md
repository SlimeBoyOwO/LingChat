---
title: Android 部署指南
description: 详细指导如何在Android设备上部署LingChat，包括手机与电脑配合使用和纯手机部署两种方案。
outline:
  level: [2, 5]
---

# 📱 Android 部署指南

本教程提供两种部署方式，请按需使用哦=w=

1. [手机+电脑的配合使用](#phone_win_deploy)
   - 适合大多数用户，使用手机和电脑配合部署。
   - 使用手机浏览器访问电脑上的 LingChat，并在电脑上运行后端服务。
2. [纯手机使用](#pure_phone_deploy)
   - 适合没有电脑或想折腾的用户。

> [!IMPORTANT]
> 在钦灵的努力下，手机端界面有了基础适配，在安装完毕后记得更新，不过界面可能仍有点奇怪。

## 一、 手机+电脑的配合使用 {#phone_win_deploy}


### 具体操作

> 请确保电脑和手机在 **同一网络** 下，否则无法使用。

> 如有需要可参考 [Windows 部署](./win.md) 教程。

首先，查看电脑 ip 地址，如果你的电脑是 Windows 系统，先在键盘上同时按下 **Windows徽标键+字母R键** 输入 **cmd** 打开命令提示符，再在黑窗口中输入 **ipconfig** ，回车，窗口中可能出现以下内容：

![cmd-ipconfig](https://lingchat.wiki/assets/depoly_android/cmd-ipconfig.webp)

记下其中的 **IPv4 地址** 后的 **ip地址**。

然后在电脑上打开 LingChat，观察命令提示符（黑窗口）中是否有这一行字：

```txt
INFO:     Uvicorn running on http://0.0.0.0:8765 (Press CTRL+C to quit)
```

记下 `0.0.0.0:` 之后的数字，这是 **端口号** 。（可能与示例不同，请以实际为准）

打开你的手机浏览器，手机调为横屏，在地址栏输入 **ip地址 + 一个英文的分号（: \) + 端口号** 即可使用。正常情况下如下图：

![手机前端演示](https://lingchat.wiki/assets/depoly_android/手机前端演示.webp)

## 二、纯手机的使用 {#pure_phone_deploy}

### 安装 ZeroTermux 环境

前往 [ZeroTermux-Github](https://github.com/hanxinhao000/ZeroTermux/releases/tag/release) 下载ZeroTermux安装包并安装。

如果下载太慢或无法下载，可尝试使用 [Github镜像源](https://ghfast.top/github.com/hanxinhao000/ZeroTermux/releases/download/release/ZeroTermux-0.118.1.43.apk) 下载并安装。

注意：**安装其他版本或者选择Termux会导致以下教程出现部分的不适用，不建议这样做**

进入ZeroTermux软件界面，提示完整阅读协议时记得要把文字内容拉到最底下。

双击屏幕左侧边缘（部分ZT版本是按音量上/下键），下滑并点击"切换源"。（推荐选择`清华源`）

等待运行完成后，再重复一遍。（让手机加深印象）

> [!NOTE] 如无特殊说明，当出现 `(Y/I/N/O/D/Z)[default=?]` 或 `[Y/N]` 时，直接点击回车，选择默认选项即可。

### 可选：解除进程限制（安卓 12 以上）

> 这一步只需安卓 12 以上版本的手机操作，如果你不清楚你的手机版本，推荐操作一下。
>
> 另外你需要打开手机开发者选项，详情搜索百度。
>
> 华为或荣耀设备请跳过此步，因为暂时用不了，请搜索：`安卓进程数量adb解限` 了解更多

首先 [安装 tmoe](/manual/deployment/android#install_tmoe)。

上下滑动屏幕选择 **修复 android 12** ，回车，看提示选择（一般全回车默认），直到下图：

![](https://lingchat.wiki/assets/depoly_android/adb地址-1.webp)

现在你需要分屏操作，分屏后点击 设置 的 **开发者选项-无线调试** 右边的滑块， 再点击左边 **无线调试** 四个大字（对没错，左边是可以点的），位置如下图：

![](https://lingchat.wiki/assets/depoly_android/adb-2.webp)

打开新界面后，点击 **使用配对码配对设备** ，弹出以下窗口，此时回到 ZeroTermux，输入 **IP 地址与端口** 中的内容，回车，再输入配对码，弹出下图：

![](https://lingchat.wiki/assets/depoly_android/adb-3.webp)

在下面的窗口点 **取消** ，上面窗口选择 **不是** 回车，然后按照下面界面另一个 **IP 地址与端口** 填上面窗口的内容，如下图：

![](https://lingchat.wiki/assets/depoly_android/adb-4.webp)

之后出现下图，配置完毕，关掉下面的设置后，在 **ZeroTermux** 回车回到主界面。

![](https://lingchat.wiki/assets/depoly_android/adb-ok.webp)

### 部署 LingChat

我们提供多种方式部署 LingChat，您可以选择最合适的进行操作。当一种方式不行时，可以更换另一种方式。

> [!WARNING]
> 方法一暂时失效，请使用方法二部署

1. [使用 tmoe 安装打包好的容器](#use_tmoe)
   - 基本上不会出现问题，最简便。

2. [使用 proot-distro 和打包好的 python](#use-proot-distro)
   - 更加轻量化，但可能有未知问题。

#### 方法一：使用预先打包好的容器 {#use_tmoe}

> 内置的 LingChat来自[我的分支](https://github.com/shadow01a/LingChat/tree/develop-termux)，因为develop分支更新比较频繁，避免更新出什么奇怪的问题......

##### 下载容器

在 **ZeroTermux 的终端** 复制执行以下命令，这将下载本人打包好的容器：

```bash
mkdir ./storage/downloads/backup
mkdir ./storage/downloads/backup/containers
mkdir ./storage/downloads/backup/containers/proot
cd ./storage/downloads/backup/containers/proot

pkg install wget

wget https://www.modelscope.cn/models/kxdw2580/LingChat-phone-file/resolve/master/debian-bookworm_arm64-LingChat-dev_2025-07-10_21-38-rootfs_bak.tar.xz

```

##### 安装tmoe {#install_tmoe}

下载完毕后，再次按下 手机音量键上（+），点击 **MOE全能** ，这里会跳出提示。

先点击回车，进入 **协议部分** ，你需要手动输入 `y` 然后回车。

接下来，你需要在选择源时选择 **gitee** 。（如果提示是：是否从 gitee 获取相关文件? [Y/n]  则回车开始安装，反之输入一个字母 **n** 再回车）

之后没有问题会进入主界面。

##### 安装 proot 和容器

在 tmoe 中选择最上面的 **proot** 回车。

等待安装完毕后，滑动屏幕选择 **恢复/还原proot容器** 回车，出现下图：

![](https://lingchat.wiki/assets/depoly_android/restore-1.webp)

选择常规模式，回车，出现以下界面：

![](https://lingchat.wiki/assets/depoly_android/restone-2.webp)

选右边那个，回车，出现以下界面：

![](https://lingchat.wiki/assets/depoly_android/restone-3.webp)

输入一个 0 ，回车，等解压完成回车回到主界面。此时再进入 proot 界面，选择当前已安装容器列表，无脑回车即可打开容器。（此时需要等待容器加载完成）

> 如果出现以下界面，直接确定。
> ![](https://lingchat.wiki/assets/depoly_android/batterychoose.webp)

OK啦，LingChat安装完毕！接下来到下面学习如何启动它。

#### 方法二：使用 proot-distro 部署 {#use-proot-distro}

##### 安装 proot-distro 及 容器

输入以下命令 **安装 proot-distro**：

```bash
pkg install proot-distro -y
```

接下来安装 **容器**：

```bash
proot-distro install debian
```

> [!NOTE] 这样安装可能会有点慢或干脆无法下载（github的锅）。此时运行以下命令安装debian：
>
> ```bash
> pkg install wget -y && 
> wget https://modelscope.cn/models/lingchat-research-studio/LingChat-phone-file/resolve/master/proot-distro-debian-bookwarm-0721.tar.xz && 
> proot-distro restore ./proot-distro-debian-bookwarm-0721.tar.xz &&
> rm -rf proot-distro-debian-bookwarm-0721.tar.xz
> ```

这时候 debian 应该安装好了，输入 `proot-distro login debian` 登录 debian。

##### apt 安装依赖

运行以下命令

```bash
# 备份 + 更换清华源 + 更新
cp /etc/apt/sources.list /etc/apt/sources.list.bak && \
tee /etc/apt/sources.list << 'EOF'
deb https://mirrors.tuna.tsinghua.edu.cn/debian/ bookworm main contrib non-free non-free-firmware
deb https://mirrors.tuna.tsinghua.edu.cn/debian/ bookworm-updates main contrib non-free non-free-firmware
deb https://mirrors.tuna.tsinghua.edu.cn/debian/ bookworm-backports main contrib non-free non-free-firmware
deb https://security.debian.org/debian-security bookworm-security main contrib non-free non-free-firmware
EOF
apt update

# 安装依赖
apt install sqlite3 git git-lfs wget curl -y

# 安装git lfs
git lfs install
```

##### 克隆 LingChat 项目文件

运行的命令有以下选择：

> [!NOTE]
> 命令加上了加速站，如有介意者自行删除使用官方源。

> [!WARNING]
> 命令2和3暂不可用，请使用另外的命令

1. 先运行 `git clone -b develop https://ghfast.top/github.com/SlimeBoyOwO/LingChat.git` 然后运行 `cd LingChat && git checkout v0.3.1-beta1 && cd` ：这会使用官方的 v0.3.1-beta1 版本，功能有所欠缺，有较多bug

2. `git clone --depth 1 -b develop https://ghfast.top/github.com/SlimeBoyOwO/LingChat/`  ：这会使用官方的 develop 分支，问题修复和功能更新更及时

3. `git clone -b develop-termux https://ghfast.top/github.com/shadow01a/LingChat/`  ：这会使用 shadow01a 的 develop-termux 分支，尽量平衡了更新进度和稳定性，且运行经过手机测试

根据你自己需求选择一条命令运行。

##### 安装 python 及其依赖

运行以下命令：

```bash
# 安装预先打包的 python3.12.10
wget https://modelscope.cn/models/lingchat-research-studio/LingChat-phone-file/resolve/master/python-3.12.10-lingchat-250707.tar.gz

tar -xzf /root/python-3.12.10-lingchat-250707.tar.gz -C /root

rm -rf python-3.12.10-lingchat-250707.tar.gz

/root/python3.12.10/bin/python3.12 -m pip install uv

```

安装完毕后即可正常使用，但为了以后简便点，我们还要做一步：

```bash
# 启动脚本
tee /root/lingchat.sh > /dev/null << 'EOF'
cd /root/LingChat
/root/python3.12.10/bin/python3.12 main.py
EOF

chmod +x /root/lingchat.sh

# 更新脚本
tee /root/update.sh > /dev/null << 'EOF'
cd /root/LingChat
git pull
/root/python3.12.10/bin/python3.12 -m uv pip install -r pyproject.toml
EOF

chmod +x /root/update.sh

```

这样以后可以用 `bash lingchat.sh` 启动 LingChat，用 `bash update.sh` 命令更新 LingChat。

### 安装情感模型

运行以下命令：

```bash
cd /root/LingChat/ling_chat/third_party/emotion_model_18emo

wget https://www.modelscope.cn/models/lingchat-research-studio/LingChat-emotion-model-18emo/resolve/master/model.safetensors

cd

```

### 配置 LingChat

> [!WARNING]
> 接下来的步骤需打开容器。
> 
> 如果你是使用 **方法一** 安装，在 ZeroTermux的终端输入 `debian` 启动安装好的容器。
> 
> 如果你是使用 **方法二** 安装，在 ZeroTermux的终端输入 `proot-distro login debian` 启动安装好的容器。

这样部署的 LingChat 不能直接使用，需要一些配置。

首先，获取 api_key 等内容，可在 [DeepSeek的官方API获取网站](https://platform.deepseek.com/) 获取。

如果使用 [硅基流动API获取网站](https://api.siliconflow.com/) 的 api_key，需要额外的env文件配置，不会的人请不要使用

然后，在容器中先粘贴以下命令，再粘贴你的 api_key ，回车运行 ：

```bash
export TRANSLATE=false && export API_KEY=
```

如果有百炼平台的 api_key，先粘贴以下命令，再粘贴你的 api_key ，回车运行 ：

```bash
export TRANSLATE=true && export BAILIAN_API_KEY=
```

之后再运行此命令：

```bash
tee /root/LingChat/.env > /dev/null << EOF
# 基础设置 BEGIN

## API 与 模型 设置 BEGIN # 配置与AI模型和API相关的密钥和地址
LLM_PROVIDER="webllm" # 在这里选择对话模型，只可以填写webllm, gemini, ollama, lmstudio四个选项，webllm代表通用需要联网的AI模型（如deepseek），ollama和lmstudio表示本地，gemini如名）
CHAT_API_KEY="$API_KEY" # DeepSeek 或其他聊天模型的 API Key
CHAT_BASE_URL="https://api.deepseek.com" # API的访问地址
MODEL_TYPE="deepseek-chat" # 使用的模型类

OLLAMA_BASE_URL="http://localhost:11434" # Ollama配置- 地址
OLLAMA_MODEL="llama3" # Ollama配置- 模型

LMSTUDIO_MODEL_TYPE="unknow" # LM STUDIO 配置- 模型
LMSTUDIO_BASE_URL="http://localhost:1234/v1" # LM STUDIO 配置- 地址
LMSTUDIO_API_KEY="lm-studio" # LM STUDIO 配置- APIKEY 似乎不需要

GEMINI_API_KEY="sk-114514"
GEMINI_MODEL_TYPE="gemini-pro"

## 视觉模型设置 BEGIN # 配置 视觉模型 相关的密钥和地址
VD_API_KEY="sk-114514" # 图像识别模型的 API Key
VD_BASE_URL="https://api.siliconflow.cn/v1" # 视觉模型的API访问地址
VD_MODEL="Pro/Qwen/Qwen2.5-VL-7B-Instruct" # 视觉模型的模型类型
## 视觉模型设置 END

## 翻译设置 BEGIN # 配置 翻译相关的密钥和地址
TRANSLATE_LLM_PROVIDER="webllm" # 翻译模型提供者（同对话，推荐webllm）
TRANSLATE_API_KEY="$BAILIAN_API_KEY" # 翻译模型的 API Key，推荐使用百炼平台的api_key
TRANSLATE_API_URL="https://dashscope.aliyuncs.com/compatible-mode/v1" # 翻译模型的api链接，推荐不要修改
TRANSLATE_MODEL="qwen3-30b-a3b-instruct-2507" # 翻译模型，推荐不要修改
## 翻译设置 END

## API 与 模型 设置 END

## 对话功能设定 BEGIN # 配置RAG（检索增强生成）系统，让AI能“记忆”历史对话
USE_RAG=false # 是否启用RAG系统 [type:bool]
USE_TIME_SENSE=true # 是否启用时间感知 [type:bool]
## 对话功能设定 END

# 基础设置 END

# 开发者设置 BEGIN

## RAG系统设定 BEGIN # 配置RAG（检索增强生成）系统，让AI能“记忆”历史对话
RAG_RETRIEVAL_COUNT=3 # 每次回答时检索的相关历史对话数量
RAG_WINDOW_COUNT=5 # 取当前的最新N条消息作为短期记忆，之后则是RAG消息，然后是过去的记忆。
RAG_HISTORY_PATH="data/rag_chat_history" # RAG历史记录存储路径
CHROMA_DB_PATH="data/chroma_db_store" # ChromaDB向量数据库的存储路径
RAG_PROMPT_PREFIX="--- 以下是根据你的历史记忆检索到的相关对话片段，请参考它们来回答当前问题。这些是历史信息，不是当前对话的一部分： ---" # RAG前缀提示，支持多行
RAG_PROMPT_SUFFIX="--- 以上是历史记忆检索到的内容。请注意，这些内容用于提供背景信息，你不需要直接回应它们，而是基于它们和下面的当前对话来生成回复。 ---" # RAG后缀提示，支持多行
## RAG系统设定 END

## 存储与日志 BEGIN # 配置日志和其他文件的存储位置
BACKEND_LOG_DIR="data/logs" # 后端服务日志目录
APP_LOG_DIR="data/log" # 应用行为日志目录
TEMP_VOICE_DIR="frontend/public/audio" # 临时生成的语音文件存放目录
ENABLE_FILE_LOGGING=false # 是否将日志记录到文件
LOG_FILE_DIRECTORY="data/run_logs" # 日志文件的存储目录
## 存储与日志 END

## Debug信息 BEGIN # 用于开发和调试的设置
LOG_LEVEL=INFO # 日志设置：默认为INFO，设置为DEBUG时启用开发者模式，输出更详尽的日志
PRINT_CONTEXT=true # 更改True/False，决定是否把本次发送给llm的全部上下文信息截取后打印到终端
## Debug信息 END

## 服务端口配置 BEGIN # 配置各个服务的网络监听地址和端口
BACKEND_BIND_ADDR="0.0.0.0" # 后端监听地址
BACKEND_PORT=12746 # 后端监听端口
FRONTEND_BIND_ADDR="0.0.0.0" # 前端监听地址
FRONTEND_PORT=3000 # 前端监听端口
EMOTION_BIND_ADDR="0.0.0.0" # 情感分析服务监听地址
EMOTION_PORT=8000 # 情感分析服务监听端口
## 服务端口配置 END


## 语音合成 BEGIN # 配置语音合成API KEY和本地地址等
SIMPLE_VITS_API_VITS_URL="http://localhost:23456/voice/vits" # SIMPLE_VITS_API(sva-vits)的语音合成API地址
STYLE_BERT_VITS2_URL="http://localhost:5000/voice" # Style-bert-vits2(sbv2)的语音合成API地址
SBV2API_API_URL="http://localhost:3000/synthesize" # Sbv2-Api(sbv2api)的语音合成API地址
SIMPLE_VITS_API_BERT_VITS2_URL="http://127.0.0.1:6006/voice/bert-vits2" # Bert-Vits2(sva-bv2)的语音合成API地址
GPT_SOVITS_API_URL="http://127.0.0.1:9880/tts" # GPT-SOVITS(gsv)的语音合成API地址
GPT_SOVITS_REF_AUDIO="" # GPT-SOVITS的参考音频路径
GPT_SOVITS_PROMPT_TEXT="" # GPT-SOVITS的参考音频文字版
AIVIS_API_KRY="" # AIVIS的API密钥
VOICE_FORMAT="wav" # 合成语音的格式，如无必要不建议修改
TTS_TYPE="" # 合成语音的引擎，可选sva-bv2,gsv,sbv2,sva-vits,sbv2api,aivis（需要角色适配）
## 语音合成 END

## 实验性功能 BEGIN # 配置实验性功能
ENABLE_EMOTION_CLASSIFIER=true # 启用/禁用情绪分类器（警告：同时关闭RAG功能后可大幅减少冷启动时间，但表情显示可能不正常）
ENABLE_DIRECT_EMOTION_CLASSIFIER=true # 是否在原有情绪可用时直接使用原标签
ENABLE_TRANSLATE=$TRANSLATE # 是否启用日语翻译功能，而不依赖于LLM的日语（需要新版人物，默认钦灵已适配）
TRANSLATE_STREAM=true # 是否启用翻译流式处理（建议开启）
OPEN_FRONTEND_APP=false # 是否在启动后端时自动打开前端应用 
USE_STREAM=true # 是否使用LLM流式生成
VOICE_CHECK=false # 是否启用语音合成检查
## 实验性功能 END

## 日程功能 BEGIN # 配置日程提醒功能（测试版）
ENABLE_SCHEDULE=false # 是否启用日程功能（包括提醒、随机聊天和窥屏功能）
## 日程功能 END

# 开发者设置 END
EOF

cd

```

这样就配置完成了。

> [!NOTE] 
> 默认未开启RAG功能，因为这必定会导致启动后第一次的白屏，需要等待加载完成刷新才行，有需要请自行安装模型再在网页打开或修改.env文件。

### 启动 LingChat

配置完成后，每次启动容器之后，就可以输入 `bash lingchat.sh` 打开 LingChat服务端，待没有东西继续输出之后，打开你的手机浏览器，手机调为横屏，在地址栏输入 `127.0.0.1:12746` 即可使用。如下图：

![](https://lingchat.wiki/assets/depoly_android/手机前端演示.webp)

### 更新 LingChat

输入 `bash update.sh` 即可自动更新。
