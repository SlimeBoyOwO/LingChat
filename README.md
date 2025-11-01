# 🐈✨ LingChat - Develop(开发版)

![official](https://github.com/user-attachments/assets/ffccbe79-87ed-4dbc-8e60-f400efbbab26)

## 项目安装教程【25/11/1 更新】

[轮椅级胎教教程](https://github.com/SlimeBoyOwO/LingChat/blob/develop/docs/develop/dev_guide.md)

## 项目结构

```txt
ling_chat
├── ling_chat          # 主包目录
│   ├── __init__.py
│   ├── api            # API 相关代码
│   ├── core           # 核心功能
│   ├── database       # 数据库相关代码
│   ├── static
│   │   ├── frontend   # 前端文件
│   │   └── game_data  # 游戏数据文件
│   ├── third_party    # 第三方集成
│   │   ├── emotion_model_18emo  # 18种情绪的情感模型
│   │   └── vits-simple-api      # 用于文本转语音的 VITS Simple API
│   ├── utils           # 工具函数
│   ├── __init__.py
│   ├── __main__.py
│   └── main.py         # 主入口点
├── data                # 用户数据文件
├── docs                # 文档文件（最新文档已迁移，此为旧版存档）
├── tests               # 测试文件
├── .env                # 环境变量文件 (用户应自己创建此文件)
├── .env.example        # 环境变量示例文件
├── .gitignore          # Git 忽略文件
├── README.md           # 项目 README 文件
└── pyproject.toml      # Poetry 配置文件
```

# 更新计划

## 服务端支持

by [Vickko](https://github.com/Vickko)

基于 LingChat 0.3 已实现的功能，使用 go 搭建服务端代码，并提供登录即用的服务。

详见[go-impl 分支](https://github.com/SlimeBoyOwO/LingChat/tree/feat/go-impl)

## 游戏引擎重构

by [风雪](https://github.com/T-Auto)

增加长线预设剧情支持，兼容肉鸽旅行/COC/DND/狼人杀等剧本呈现方式，且原生兼容读档存档、多人物同屏和记忆库系统的底层框架。

详见 Issues：[【0.4.0 开发日志】长剧情系统＆多角色同屏＆随机事件演进＆小游戏框架](https://github.com/SlimeBoyOwO/LingChat/issues/91)，源码位于仓库[NeoChat](https://github.com/T-Auto/NeoChat)，剧情方面参考[NeoChat 剧情创作指南](https://github.com/T-Auto/NeoChat/blob/main/%E5%89%A7%E6%83%85%E5%88%9B%E4%BD%9C%E6%8C%87%E5%8D%97.md)

## 记忆系统重构

by [云](https://github.com/LtePrince)

重构记忆系统，使用图数据库实现 RAG 来提升性能。

详见 Issues：[【0.4.0 开发日志】基于图数据库实现 RAG](https://github.com/SlimeBoyOwO/LingChat/issues/82)，源码位于仓库[LongTermMemoryRAG](https://github.com/LtePrince/LongTermMemoryRAG)

## 新的 UI

by [yukito](https://github.com/yukito0209)、[喵](https://github.com/a2942)

更好看的启动 UI！

详见 Demo：[main_page_demo](https://github.com/SlimeBoyOwO/LingChat/tree/develop/Demo/main_page_demo)

## 模块化的 api 兼容层

by [uwa](https://github.com/myh1011)

将任意 api 转为标准 openai 格式，实现对各种 api 的系统性支持。

详见 Demo：[EPU-Api](https://github.com/SlimeBoyOwO/LingChat/tree/develop/Demo/epu-api)

或 github [EPU-Api](https://github.com/myh1011/epu_api)

## 桌宠启动方式

by [dada](https://github.com/kono-dada)

提供轻量化的桌宠启动方式。

现已迁移至：[Ling-Pet 项目](https://github.com/kono-dada/Ling-Pet)

## 多语言框架

by [Thz922](https://github.com/Thz922)

为 LingChat 添加多语言支持。

详见 Issues：[为项目添加多语言支持 · Issue #129 · SlimeBoyOwO/LingChat](https://github.com/SlimeBoyOwO/LingChat/issues/129)

## 安卓端开发

by [shadow01a](https://github.com/shadow01a)

探索安卓端的使用。

目前已经有了[可用的文档](https://lingchat.wiki/manual/deployment/android_deploy.html)

## 文档

by [foxcyber907](https://github.com/foxcyber907)

拆分文档部分并独立更新。

详见网站 [LingChat Wiki](https://lingchat.wiki/) 或者 [GitHub 仓库](https://github.com/foxcyber907/ling-docs)

## 前端重构

使用 vue 彻底重构前端。

详见[frontend_vue](https://github.com/SlimeBoyOwO/LingChat/tree/develop/frontend_vue)
