---
title: Clone后修复指南
description: 详细指导Clone 项目文件后，文件未追踪导致大量文件显示需要 commit 的问题
---

## 问题描述

Clone 项目文件后，文件未追踪导致大量文件显示需要 commit

## 问题原因

LFS 在开启代理的状态或者不稳定时，会出现证书验证无效导致.git 文件损坏，从而导致项目所有文件无法追踪的问题

## 解决方法

刷新 Git 文件，人工初始化即可：

1. 删除原来的 `.git` 文件夹
2. 在项目根目录输入下面的指令

```
git init
git remote add origin https://github.com/SlimeBoyOwO/LingChat.git
git fetch
git reset --hard origin/develop
```

3. 切换到 develop 分支