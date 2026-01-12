# Linux部署
#### 本文旨在完善LingChat对linux系统的开发环境部署教程方面的欠缺
##### 如果你只是想使用LingChat请移步  [Linux 用户部署教程](Linux_user.md)


## 0 前往[LingChat官方仓库](https://github.com/slimeBoyOwO/LingChat/) fork一份源码至您的帐户中，这里不做详解
## 1 拉取项目源码
- 1.1 在您的设备上建立一个文件夹用于存放项目源码
- 1.2 安装适用于您的系统的git和git lfs，具体过程不做详解
- 1.3 请使用 ``git clone -b develop https://github.com/your_name/LingChat.git ``克隆仓库
## 2 创建虚拟环境(Python venv)
- 2.1 进入项目文件夹，运行``python -m venv venv``，等待完成后使用``source ./venv/bin/activate``激活环境
- 2.2 使用``pip install .``安装依赖

### 3 运行项目根目录的main.py即可
### 4 前端部分请自行阅读frontend_vue/start.bat，在linux下复现即可部署
