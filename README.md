## 该docker部署问题修复差不多了

1. 已知问题：docker的python端不会有日志输出，很奇怪

## 运行前配置

把`.env.example`复制为`.env`并填写里面的`CHAT_API_KEY`.

## docker运行配置

把`.env.example.docker`复制为`.env.docker`并填写里面的`CHAT_API_KEY`.

## 打包相关

新建了WebChat.exe.py。WebChat.exe.py保持原功能不变，但是现在他运行后，会在python代码内部直接启动前端server.js和浏览器窗口（在之前，这两者依靠.bat启动），为后续编译python环境做准备。

由于WebChat.exe.py实现了原start.bat的部分功能，现在新增start-new.bat，在创建虚拟环境后只会启动WebChat.exe.py单个文件，而不再启动server.js和浏览器窗口。

现在可以对项目进行编译了，并且好消息是，现在他只会弹出一个终端运行窗口，更加的美观