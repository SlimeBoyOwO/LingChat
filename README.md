## 该docker部署问题修复差不多了

1. 已知问题：docker的python端不会有日志输出，很奇怪

## 运行前配置

把`.env.example`复制为`.env`并填写里面的`CHAT_API_KEY`.

## docker运行配置

把`.env.example.docker`复制为`.env.docker`并填写里面的`CHAT_API_KEY`.

## 打包相关

现在把windows程序和docker程序分开了。

windows程序对应的后端是Webchat.windows.py，现相比之前，现在Webchat会在python代码内部直接启动前端server.js和浏览器窗口（在之前，这两者依靠.bat启动），为后续编译python环境做准备。

- 对应start.windows.bat启动

原Webchat.py名称修改为Webchat.docker.py，不改变其功能，这是因为docker分前端和后端。

- 对应start.docker.bat启动

现在可以随时对项目进行编译了，可以轻松打包环境，点击exe即可启动。并且好消息是，现在他只会弹出一个终端运行窗口，更加的美观