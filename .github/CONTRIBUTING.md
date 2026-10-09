# 贡献指南

由衷的感谢您有兴趣为 **LingChat** 的发展做出贡献！在作出代码贡献之前，请花几分钟时间阅读本指南。

## 一、 AI 开发指南

新时代 AI 中，开发者使用 AI 几乎是必然的，确保你对 AI 生成的代码有审核的能力，并保证提交的代码不会造成：

1. **过量的代码膨胀，无法审核的代码量**
2. 在项目已有相关逻辑，AI 重复生成相关逻辑
3. 提交时提交**简洁的改动声明**，不要把 AI 大量包含具体函数和文件名的解释提交，说明重点。

## 二、贡献步骤

### 1’ 对于 bug 类型修复的贡献：

1. 如有能力，无需创建 Issue，直接提交 Pull Request，说明修复的bug
2. 也欢迎在 issue 汇报 bug

### 2’ 对于小规模的新功能/优化：

1. 如果代码量较小（小于 500 行左右），可以直接提交 Pull Request，但请务必在 Pull Request 中详细描述你的改动。
2. 如果代码量较大，**强烈不建议直接在一次 Pull Request 提交你的所有改动，尤其是 AI 生成**，可以分多次提交，每次提交一个独立的功能或优化。

### 3’ 对于大规模的新功能/重构：

1. **请先创建 Issue**，详细描述你的新功能或重构计划，并说明它的价值和意义。与开发者讨论。
2. 无 Issue 即提交代码，**极有可能导致需求不对等，实现上由于没讨论导致实现方式错误，代码质量不过关的情况**。

## 三、项目需要重点注意的地方

### 1. 人工维护与AI开放部分

- 对于核心模块，如数据库，主消息回复逻辑`generator.rs`，核心记忆构建器算法`memory_builder.rs`，`ai_service`的`game_status`里等相关重要模块后端重要代码部分，在开发相关模块时，**必须人工仔细检查并测试**，这一部分的代码在审核时会被重点审查。
- 对于前端（除了核心消息队列处理逻辑），许多边缘功能代码，允许使用 AI 生成，但起码要保证人工测试通过，并且代码不存在太离谱的错误。在实现华丽的，有创意的新功能时搭配 AI，允许一定界内的代码不完美度。

### 2. 项目提示词原则

- 本项目的提示词全部搭建在核心记忆构建器算法之上，无论是玩家输入，系统提示，旁白，全部都是通过向`game_status`的台词表添加内容实现的。并且以**讲故事的方式的提示词风格**来保证角色没有人机味。开发的时候务必注意这一点。
- 确保不要添加第二条 `system` 类型的台词在后续的上下文。

### 3. 和开发者们一起交流吧

- 由于本项目是学习项目，也出于和开发者们更方便沟通的目的，可以加入我们的开发者群一起讨论，QQ群号：798012738

---

## 💻代码开发

### 配置系统

为开发项目，您必须安装并配置系统的 Node 和 Rust 环境，并且这里推荐您使用 VSCode 或者 VSCodium 作为代码编辑器。

- 安装 Node ==> [Node 官方网站](https://nodejs.org/zh-cn)
- 安装 Rust ==> [Rust 官方网站](https://rust-lang.org/zh-CN)
- 安装 VSCode ==> [VSCode 官方网站](https://code.visualstudio.com/)
- 安装 VSCodium ==> [VSCodium Gihtub](https://github.com/VSCodium/vscodium)

### 克隆项目

```shell
git clone https://github.com/SlimeBoyOwO/LingChat

cd LingChat
```

### 编译调试

如果您使用的是 Linux 系统可能需要安装如下包

```shell
sudo pacman -S clang
```

如果 node 没有下载 pnpm 您需要进行下载

```shell
npm install -g pnpm
```

下载外部资源

```shell
pnpm install
pnpm run init
```

只编译前端

```shell
pnpm install
pnpm run build
```

测试运行

```shell
pnpm run tauri dev
```
