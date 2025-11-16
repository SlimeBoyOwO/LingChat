---
title: 界面与启动排障
description: 解决LingChat界面显示问题和启动问题，包括加载问题、端口占用问题和浏览器兼容性问题。
---

# 🔳界面与启动排障

本板块收录 LingChat 启动、界面、端口等相关常见问题及解决方法。

## 启动问题

### Lingchat 启动半天都在转圈圈

![3c06ff9da6aa261ba2855d0c512f656e](https://lingchat.wiki/assets/faq-screen/463052615-ca13a991-4e8a-4c0d-b98e-67be36845a79.webp)

- 多等一会，初始化时间比较长，在此期间不要关闭，如果最后还是无法启动，那没救了（，你的电脑可能是 20 多年前的老古董

### 通常每个套接字地址(地址/网络地址/端口)只允许使用一次

![image](https://lingchat.wiki/assets/faq-screen/463053323-ea51c143-12da-4c7a-9c5e-8f18bc54b673.webp)

- 可能你的上一个 Lingchat 没有关闭，或者有其他从程序占用了 8765 这个端口，你可以检查是哪个程序占用的，把它那个程序给关了（或者使用重启大法）

### `d91mError: Failed to initialize file logging: [Errno 13] Permission denied: 一个路径` 或者 `[WinError 5]：拒绝访问：一些英文`

![image](https://lingchat.wiki/assets/faq-screen/path.webp)

在 安装 或 解压 LingChat时，请检查 **不要让 LingChat 在C盘等特殊位置** 

你可以将 LingChat 安装在除 C盘 以外的其它盘或者桌面

Linux等用户请检查 LingChat 文件夹的读写权限等有没有给

## 对话问题

### 提示 402 错误: Your api key is invalid

![9049a8c0adc590f56717b4fec3f705f6](https://lingchat.wiki/assets/faq-screen/463052225-26c603e7-0ef2-4664-903a-11a366720449.webp)

- 翻译：你的 API 无效，检查设置里有没有改过秘钥
- 检查是否 API KEY 和 URL 是对应的，硅基流动的秘钥不能给 deepseek 用之类的

### 提示 402 错误：Insuffient Balance

![36424a8d6fc3b28ae50e004a1ef31356](https://lingchat.wiki/assets/faq-screen/463052051-68e96fb7-38e1-465f-bb7e-2ab6f0c5c942.webp)

- 翻译：你没有余额，假如你用 deepseek，要先充值

## Simple Vits API 语音问题

### 启动提示 py 有问题

![2da141f2e7c7e654c979ccbbde19d5d6](https://lingchat.wiki/assets/faq-screen/463051728-7b560c32-8831-4dcc-b313-eb4be55f86c9.webp)

- 如果你在解压的时候遇到了错误提示，则软件是无法打开的，推荐使用 Bandzip 解压软件，无广告而且比原版 windows 好用多了。然后使用 Bandzip 解压，无错误则成功

### Vits 卡住，提示 Connection 拒绝

![1c3045c784d4bd9c1d2ffbe9070b01ae](https://lingchat.wiki/assets/faq-screen/463047579-2ab88016-4efa-4bab-90aa-d952e83f5ed8.webp)

- 打开 localhost:23456，随便生成一个语音。如果有错误提示：
- 使用梯子，确保网络环境通畅，首次生成语音会下载国外的东西。
- 如果网页能正常生成语音，则 lingchat 肯定也没问题
- 重启 lingchat，即可修复

### 提示 speaker_id 不存在

![375cec5632c886575ef240d06897302e](https://lingchat.wiki/assets/faq-screen/463047395-5d0d1e7c-14a4-43fb-b240-2299bcb4cf45.webp)

- 没有安装语言模型，把语音模型下载到 data/models/文件夹
- 如果使用的是其他 vits 模型，请人工设置 game_data/characters/人物名/settings.txt 里面的 speaker_id 的值，可以用 0 测试

### MeCab初始化失败

![2525ef209761d9662ef70c047feab802](https://lingchat.wiki/assets/faq-screen/463047395-5d0d1e7c-14a4-43fb-b240-2299bcb4cf45.webp)

- Vits有中文路径导致的，不要包含任何中文路径

### 生成之后卡死，无报错提示

- 经过测试，部分50系显卡会有这个问题，simple-vits-api似乎暂时不支持部分50系显卡，请使用其它语音项目

## Style Bert Vits2 语音问题

> 这个问题太多了，可以在群里直接问相关问题，或者使用sbv2api项目

## 界面问题

### 界面一片空白没有人物

程序加载较慢（尤其是打开rag时），只需等待片刻即可。

当然，如果等待时间过久，请汇报程序有问题