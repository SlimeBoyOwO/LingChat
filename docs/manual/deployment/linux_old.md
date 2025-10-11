---
title: Linux 旧版部署教程
description: 学习如何在Linux系统上部署旧版LingChat，包括环境配置、依赖安装和部署步骤指南。
outline:
  level: [2, 4]
---

# 🐧 Linux 部署

以下内容假设你对Linux系统有一定的了解，如果觉得难以理解，请使用[Windows部署](./win_old.md)

> [!NOTE]
> 本教程推荐使用 [uv](https://docs.astral.sh/uv/) 作为 Python 包管理器，它提供了更快的包安装速度和更好的依赖管理体验。当然，传统的 pip 和 conda 方式依然可用。
> 不要忘记为 uv 配置镜像站，如果有需要的话

> [!NOTE]
> 我们建议使用 mamba 或 micromamba 代替 conda ，它与 conda 使用方式几乎相同，并在处理依赖上优于 conda。

## 一、克隆LingChat，获取必要的文件

通过 git clone 将 [LingChat repo](https://github.com/SlimeBoyOwO/LingChat/) clone 到本地，再进入文件夹。

```bash
git clone -b main https://github.com/SlimeBoyOwO/LingChat.git
```

> [!TIP]
> 如果您想提前体验新功能，可[看新版教程](./linux.md)

## 二、Python环境配置

需确保Python版本为3.10及以上，推荐3.12

```bash
python3 --version
```

如果版本低于3.10，请更新Python版本。

```bash
# 此处以 Python 3.12 为例
# Ubuntu/Debian
sudo apt update
sudo apt install python3.12 python3.12-venv

# 如执行了这一步，建议在执行时将python3指向python3.12
# 更新替代方案，设置 python3.12 为默认的 python3 版本:
sudo update-alternatives --install /usr/bin/python3 python3 /usr/bin/python3.12
sudo update-alternatives --config python3
```

### 安装 uv (推荐)

安装 uv 包管理器：

```bash
# 使用 pip 安装 uv
pip3 install uv
```

或者使用官方安装脚本：

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

> [!WARNING]
> 有[报告指出](https://github.com/foxcyber907/ling-docs/issues/12)使用 `Fedora 42 Workstation` 系统时，uv会无法使用。你可以尝试执行以下命令：
> ```
> sudo dnf install gcc gobject-introspection-devel cairo-gobject-devel pkg-config python3-devel gtk4
>```

### 传统方式环境配置

#### 方法1：使用venv

```bash
python3 -m venv venv      # 创建虚拟环境    
source venv/bin/activate  # 激活环境
```

#### 方法2：使用conda/mamba

```bash
mamba create -n LingChat python=3.12
mamba activate LingChat
```

## 三、依赖安装

> [!WARNING]
> 不要忘记激活虚拟环境！无论何时你都不应该脱离虚拟环境操作
>
> 如果终端前有（venv）或（lingchat）字样即为虚拟环境已经激活

### 使用 uv 安装依赖 (推荐)

```bash
uv pip install -r requirements.txt -i https://mirrors.aliyun.com/pypi/simple
```

> [!NOTE]
> `uv pip install` 在该环境中安装依赖时貌似有个错误，但是我忘了是啥了，欢迎 [提issue](https://github.com/foxcyber907/ling-docs/issues)

### 使用传统方式安装依赖

```bash
pip install -i https://mirrors.aliyun.com/pypi/simple -r requirements.txt
```

### 配置 `.env` 文件 {#env_file}

```bash
mv .env.example .env
nano .env
```

根据提示配置 `API密钥` 即可

配置完成后请按`Ctrl+O`保存，然后按`Ctrl+O`退出。

> [!NOTE] 默认未开启RAG功能，因为这必定会导致启动后第一次的白屏，需要等待加载完成刷新才行，有需要请自行在网页打开或修改.env文件。

## 四、模型下载

LingChat 需要下载一些模型才能正常使用。

### 18 情绪分类模型

运行以下命令：

```bash
cd backend/emotion_model_18emo
wget https://www.modelscope.cn/models/kxdw2580/LingChat-emotion-model-18emo/resolve/master/model.safetensors
```

### RAG 模型

先激活虚拟环境，然后运行以下命令：

```bash
python3 backend/core/memory_rag/downloading.py
```

## 启动程序

确保已经激活虚拟环境

### 前台运行

```bash
python backend/windows_main.py
```

### 后台运行

如需在后台运行请使用screen

```bash
# 启动一个screen
screen -S lingchat
source ./venv/bin/activate  # 激活环境
# 运行lingchat
python backend/windows_main.py
```

> 按`Ctrl+a`, 再按`d`, 即可退出screen, 此时,程序仍在后台执行;  

## 四、访问 LingChat

放行端口 8765 端口，或者使用ssh端口转发。
在浏览器中访问 `http://<你的服务器IP地址>:8765` 即可访问LingChat。

> [!TIP]
> 将LingChat部署在公网是非常危险的行为，可能导致您的api被盗刷，请务必在部署时进行安全配置。

## 五、拉取最新的更新

目前main分支已不会再收到最新的功能和问题修复，无需拉取

## 命令速查表

### uv 相关命令 (推荐)

| 命令 | 用途           |
|------|--------------|
| `uv venv` | 创建Python虚拟环境 |
| `uv pip install -r requirements.txt -i https://mirrors.aliyun.com/pypi/simple --upgrade` | 安装依赖包        |
| `python backend/windows_main.py` | 运行LingChat程序 |

### 传统方式命令

| 命令                           | 用途 |
|------------------------------|------|
| `source ./venv/bin/activate` | 激活Python虚拟环境（使用venv） |
| `conda activate LingChat` | 激活Python虚拟环境（使用conda） |
| `python backend/windows_main.py`             | 运行LingChat程序 |

### 后台运行相关

| 命令                   | 用途                              |
|----------------------|---------------------------------|
| `screen -S lingchat` | 创建一个名为lingchat的screen会话运行LingChat程序   |
| `Ctrl+a d`           | 退出当前screen会话(程序继续在后台运行)         |
| `screen -r lingchat` | 重新连接到mmc会话                      |
| `screen -ls`         | 查看所有screen会话列表                  |
