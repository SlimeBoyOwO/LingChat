## 该docker部署问题修复差不多了

1. 已知问题：docker的python端不会有日志输出，很奇怪

## 运行前配置

把`.env.example`复制为`.env`并填写里面的`CHAT_API_KEY`.

## docker运行配置

把`.env.example.docker`复制为`.env.docker`并填写里面的`CHAT_API_KEY`.

## 打包相关

新建了WebChat.exe.py，该py现在运行后会直接启动前端js和浏览器窗口，为后续编译python环境做准备。

原start.bat会启动前端js和浏览器窗口，现在start-new.bat现在只会启动WebChat.exe.py单个文件
