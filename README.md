# 灵聊 简单的webui聊天程序！

> 通过python和nodejs和前端代码实现的简单对话功能。

## 功能列表
- ✅ 随时和用户对话，可以使用表情，动作和聊天气泡。
- ✅ 在logs中记录你们聊天的记录。
- ✅ 在菜单更改设置并且浏览当前历史记录。

## 如何使用？
1. 下载好仓库内的东西，确保你下载了nodejs和python环境
2. 点击backend/install.bat安装必要的库（该库是全局安装的！！如果你电脑已经有python环境了谨慎操作）
3. 从网盘下载情感分类模型，放在backend/emotion_model_12emo中
4. *由于隐私原因，请在deepseek.py输入自己的api或者自己改写程序接入其他api*
5. 启动backend中的run.bat，启动根目录的run_server.bat即可启动
6. 输入localhost:3000 进入聊天界面，左上角显示已连接服务器则表示完成
7. 为了使用语音功能，请前往链接下载vits链接程序！程序默认监听23456语音端口
8. 程序默认导入的模型是zcchat地址->讨论区->角色示范（丛雨）->vits模型下载好之后在simple-vits-api的目录的/data/models里面解压，再启动就ok了
9. 如果需要使用其他模型，在webChat.py的Vits实现函数更改相关设定即可

## 相关设定
1. 在deepseek.py里的settings设定角色性格和你的设定（别忘了自己的API一定要填写）
2. 可以更换/public/pictures/lingling/里面的立绘+修改/public/css/galgame.css里的代码实现自定义角色或表情动作气泡
3. /public/js/talk.js 里面可以设定不同的心情和不同的动作，目前有12种情绪，由于模型是自己训练的所以更新要等一段时间啦

## 相关链接
- [emotion_model_12emo](https://pan.baidu.com/s/16Dy53KX3jIjACY5fCctKDA)：请在这里下载emotion_model_12emo，提取码：0721
- [simple-vits-api](https://github.com/Artrajz/vits-simple-api): 该项目实现了基于 VITS 的简单语音合成 API。建议下载GPU版本，速度快
- [zcchat](https://github.com/Zao-chen/ZcChat): 本项目的灵感来源，可以在这里找到Vits模型和人物素材

## 一些小话
- 本项目为了快速开发用了很多AI工具，有做的不好的地方欢迎指出！
- 会随着项目的知名度提供更便利清晰的自定义功能的！目前实在没时间啦...
- 本项目更多作为一个超小型的学习项目，由于文件结构非常简单，欢迎有兴趣的人学习。

## 其他
> 本项目使用的气泡+音效素材来源于碧蓝档案，请勿商用
> 默认简单狼狼立绘是自绘，表情差分源于AI，如果你想自己创作可使用 Novelai 网站
> 有其他问题可以B站私信捏

© 诺一 钦灵
