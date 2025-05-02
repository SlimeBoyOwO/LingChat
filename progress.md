# 项目进展 (progress.md)

## 当前目标：

发布适合小白的，无报错的一键可运行版本

## 进行中:

- 提高编译版本启动方式的兼容性
- 提高docker run启动方式的兼容性
- 提高Linux环境下部署的兼容性
- 扩展到18情绪分类器

## 计划:

- 在README新增docker部署教程，修订Linux部署教程
- 打包python环境或发布编译版本，增强环境兼容性

## 已知问题/风险:

- docker的python端不会有日志输出，很奇怪
- 打开菜单动画有概率会和galgame社精一样闪屏，原因未知

## 弃用计划：
- 将前端和后端合并为同一个Docker镜像（不符合规范&&有潜在风险）



## 已完成:

- .bat一建启动文件的编写
- 一键docker run的编写
- 项目路径整理
- 快速点击发送键会导致AI输出进入用户输入，并截成好多段，自动发送大量请求
- 完成了项目的编译准备，可随时打包整个python环境，以编译过的exe启动，提高兼容性，防止小白无法安装环境。目前打包后的总大小（包括情绪分类模型）为500mb。
- **把windows程序和docker程序分开，分别为Webchat.windows.py和Webchat.docker.py。Webchat.windows.py会在python代码内部直接启动前端server.js和浏览器窗口**
- **将可配置项移动到了根目录下的.env，请于`.env.example.docker`和`.env.example.docker`中查看配置示例。**
- 优化了网页性能
- 修复EmoPredictBatch并发未关闭channel导致死锁的bug，以及typo
- 添加go backend README, 将gomod、dockerfile和README对golang的版本要求统一降至1.21
- 优化了动画兵添加了更多动画

## main上的兼容性改进，其他分支也应注意此问题

- [main]修改了requirements.txt的torch版本为2.6.0，以防镜像站没有2.0.1版本导致pip失败
- [main]修改了start.bat文件的编码为UFT-8，换行符为CRLF，提高兼容性
- [main]backend路径问题。若使用start.bat启动，需要在predictor.py导入情绪模型的路径中添加一层/backend，这是因为start.bat和WebChat.py运行的根目录不一致，后续更新请注意启动路径问题。
