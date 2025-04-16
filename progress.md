# 项目进展 (progress.md)

## 当前目标：

发布适合小白的，无报错的一键可运行版本



## 已完成:

- .bat一建启动文件的编写
- 一键docker run的编写
- 项目路径整理

## Debug:

- [main]修改requirements.txt的torch版本为2.6.0，以防镜像站没有2.0.1版本导致pip失败
- [main]修改start.bat文件的编码为UFT-8，换行符为CRLF，提高兼容性

## 进行中:

- 验证.bat启动方式在各种电脑均可一键启动
- 验证docker run启动方式的兼容性
- 验证Linux环境下的兼容性
- [main]backend路径问题



## 计划:

- 将所有变量移动至config配置文件
- 将前端和后端合并为同一个Docker镜像
- 在README新增docker部署教程，修订Linux部署教程
- 打包python环境或发布编译版本，增强环境兼容性



## 已知问题/风险:

- 快速点击发送键会导致AI输出进入用户输入，并截成好多段，自动发送大量请求
