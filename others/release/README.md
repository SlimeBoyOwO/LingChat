# 打包工具

## 说明

目前本打包工具的逻辑是：

- 依据requirements生成一个不依赖系统环境的独立python
- 使用这个独立python启动项目
- 把使用这个独立python启动项目的.bat脚本编译成.exe

## 用法

- 把build.py放到根目录
- 会依据requirements自动生成一个独立python——注意，最好是安装torch-cpu
- 鉴于文件结构没温稳定，如果要发布release版本，请手动将源码复制出来，并删除Logs/RAG缓存/.env等敏感信息
- 如果想法release的话，就可以按照这个流程打成zip并发送