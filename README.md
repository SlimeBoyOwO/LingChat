# 🐈✨ LingChat - Develop(开发版)

![official](https://github.com/user-attachments/assets/ffccbe79-87ed-4dbc-8e60-f400efbbab26)

## Installation
### Installation via pip
you may want to install the package in a virtual environment to avoid conflicts with other packages. You can create a virtual environment using `venv` or `virtualenv`.
```bash
python -m venv .env
```
then activate the virtual environment:
```bash
source .env/bin/activate  # On Linux or macOS
.env\Scripts\activate  # On Windows
```
you can install the package using pip:
```bash
pip install .
```

### Installation via poetry
you can install the package using poetry:
```bash
poetry install
```

## Usage

### add your key
create a file named `.env` in the root directory of the project and add your Chat API key to it:
```text
CHAT_API_KEY="sk-<here-is-your-key>"
```

### run the server
run the following command to start the server:
```bash
# if you installed the package via pip
# python -m py_ling_chat
# or: 
poetry run python -m py_ling_chat
```

## Project Structure
```
py_ling_chat
├── py_ling_chat  # Main package directory
│   ├── __init__.py
│   ├── api  # API-related code
│   ├── core  # Core functionality
│   ├── database  # Database-related code
│   ├── static
│   │   ├── frontend  # Frontend files
│   │   └── game_data  # Game data files
│   ├── third_party  # Third-party integrations
│   │   ├── emotion_model_18emo  # Emotion model for 18 emotion
│   │   └── vits-simple-api  # VITS Simple API for text-to-speech
│   ├── utils  # Utility functions
│   ├── __init__.py
│   ├── __main__.py
│   └── main.py  # Main entry point
├── data  # User Data files
├── docs  # Documentation files
├── tests  # Test files
├── .env  # Environment variables file (user should create this)
├── .env.example  # Example environment variables file
├── .gitignore  # Git ignore file
├── README.md  # Project README file
└── pyproject.toml  # Poetry configuration file
```

# 更新计划

## 服务端支持

by [Vickko](https://github.com/Vickko)

基于LingChat 0.3已实现的功能，使用go搭建服务端代码，并提供登录即用的服务。

详见[go-impl分支](https://github.com/SlimeBoyOwO/LingChat/tree/feat/go-impl)

## 游戏引擎重构

by [风雪](https://github.com/T-Auto)

增加长线预设剧情支持，兼容肉鸽旅行/COC/DND/狼人杀等剧本呈现方式，且原生兼容读档存档、多人物同屏和记忆库系统的底层框架。

详见Issues：[【0.4.0开发日志】长剧情系统＆多角色同屏＆随机事件演进＆小游戏框架](https://github.com/SlimeBoyOwO/LingChat/issues/91)，源码位于仓库[NeoChat](https://github.com/T-Auto/NeoChat)，剧情方面参考[NeoChat剧情创作指南](https://github.com/T-Auto/NeoChat/blob/main/NeoChat%20%E5%89%A7%E6%83%85%E5%88%9B%E4%BD%9C%E6%8C%87%E5%8D%97.md)

## 记忆系统重构

by [云](https://github.com/LtePrince)

重构记忆系统，使用图数据库实现RAG来提升性能。

详见Issues：[【0.4.0开发日志】基于图数据库实现RAG](https://github.com/SlimeBoyOwO/LingChat/issues/82)，源码位于仓库[LongTermMemoryRAG](https://github.com/LtePrince/LongTermMemoryRAG)

## 新的UI

by [yukito](https://github.com/yukito0209)、[喵](https://github.com/a2942)

更好看的启动UI！

详见Demo：[main_page_demo](https://github.com/SlimeBoyOwO/LingChat/tree/develop/Demo/main_page_demo)

## 模块化的api兼容层

by [uwa](https://github.com/myh1011)

将任意api转为标准openai格式，实现对各种api的系统性支持。

详见Demo：[EPU-Api](https://github.com/SlimeBoyOwO/LingChat/tree/develop/Demo/epu-api)

或github [EPU-Api](https://github.com/myh1011/epu_api)

## 桌宠启动方式

*当前无固定人员开发

添加轻量化的桌宠启动方式。

详见Demo：[desktop_pet](https://github.com/SlimeBoyOwO/LingChat/tree/develop/Demo/desktop_pet)

## 多语言框架

by [Thz922](https://github.com/Thz922)

为LingChat添加多语言支持。

详见Issues：[为项目添加多语言支持 · Issue #129 · SlimeBoyOwO/LingChat](https://github.com/SlimeBoyOwO/LingChat/issues/129)



## 安卓端开发

by [shadow01a](https://github.com/shadow01a)

探索安卓端的使用。

详见Issues：[关于新增手机使用本项目的文档 · Issue #128 · SlimeBoyOwO/LingChat](https://github.com/SlimeBoyOwO/LingChat/issues/128)

目前已经有了一个[可用的文档](https://github.com/SlimeBoyOwO/LingChat/blob/develop/others/document/%E6%89%8B%E6%9C%BA%E4%BD%BF%E7%94%A8%E6%95%99%E7%A8%8B.md)
