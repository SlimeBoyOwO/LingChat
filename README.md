# [0.1.0-正式版] - 待发布(develop)
已更新：

- 将12类情感分类模型扩展，新增18类情感分类模型

计划：

1，多用户支持
- 为服务器端部署增加用户注册与登录功能

2，聊天记录切换
- 支持用户在浏览器端创建、切换不同的聊天会话（上下文）。

3，人设切换与分享
- 支持用户在浏览器端创建、切换不同的人设+背景剧情Prompt。
- 支持用户导出/导入人设+背景剧情Prompt的json

4，双部署模式
- 项目按照服务器部署模式开发
- 为个人本地部署模式提供一建包：实质是第一次启动自动注册，后续启动时以本地用户账号登录。同时在README中提供本地账户默认的用户名和密码

当前项目结构
```
LingChat/
├── Dockerfile-node # Node.js环境Docker配置
├── Dockerfile-predictor # 情感预测服务Docker配置
├── Dockerfile-python # Python环境Docker配置
├── backend/ # 后端服务目录，包含核心业务逻辑和AI交互实现。
│   ├── VitsTTS.py # 基于VITS的文本到语音转换模块
│   ├── deepseek.py # DeepSeek AI模型接口
│   ├── emotion_model_12emo/ # 12分类情感模型目录
│   ├── emotion_model_18emo/ # 18分类情感模型目录
│   ├── go-impl/ # Go语言实现的组件
│   ├── langDetect.py # 语言检测模块，用于区分中日文本
│   ├── logger.py # 日志记录模块
│   ├── predictor.py # 情感分类模型，使用BERT分析文本情感
│   ├── predictor_server.py # 情感预测服务的独立服务器
│   ├── run.bat # Windows环境下的启动脚本
│   ├── webChat.docker.py # Docker环境下的主程序入口
│   ├── webChat.windows.py # Windows环境下的主程序入口，处理WebSocket连接、AI回复处理和情感分析
│   └── webChat2.py # 主程序的替代版本
├── docker-compose.yml # Docker容器配置
├── frontend/ # 前端应用目录，基于Node.js和WebSocket实现实时通信。
│   ├── package.json # 项目依赖配置
│   ├── public/ # 前端静态资源目录
│   │   ├── audio/ # 语音文件存储目录
│   │   ├── audio_effects/ # 音效文件目录
│   │   ├── css/ # 样式表文件
│   │   │   ├── animation.css # 动画效果
│   │   │   ├── browser-compat.css # 浏览器兼容样式
│   │   │   ├── galgame.css # 主要样式
│   │   │   └── menu.css # 菜单样式
│   │   ├── js/ # JavaScript文件
│   │   ├── pages/ # HTML页面文件
│   │   │   └── index.html # 主页面，包含对话界面和设置菜单
│   │   └── pictures/ # 图片资源目录，包含背景和角色图片
│   ├── run_server.bat # 前端服务器启动脚本
│   └── server.js # 前端服务器，处理静态资源和WebSocket消息转发
├── logs/ # 日志文件目录
├── requirements.txt # Python依赖列表
├── scripts/ # 脚本工具目录
│   └── start.py # 项目启动脚本，负责启动虚拟环境、检查依赖、启动各组件服务
├── start.docker.bat # Docker环境启动脚本
├── start.windows.bat # Windows环境启动脚本
├── temp_voice/ # 临时语音文件存储目录
└── third_party/ # 第三方依赖目录
    └── install.bat # 第三方依赖安装脚本
```

0.2.0计划：

使用RAG向量库等实现长期记忆
增加类似安科/跑团式的长线预设剧情支持
