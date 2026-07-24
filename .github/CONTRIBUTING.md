# 贡献指南

首先，很高兴你能看到这里，由衷的感谢您有兴趣为 **LingChat** 的发展做出贡献！每一份来自社区的力量，都是项目不断前进的宝贵力量。

## 💻代码开发

### 配置系统

为开发项目，您必须安装并配置系统的 Node 和 Rust 环境，并且这里推荐您使用 VSCode 或者 VSCodium 作为代码编辑器。

- 安装 Node ==> [Node 官方网站](https://nodejs.org/zh-cn)
- 安装 Rust ==> [Rust 官方网站](https://rust-lang.org/zh-CN)
- 安装 VSCode ==> [VSCode 官方网站](https://code.visualstudio.com/)
- 安装 VSCodium ==> [VSCodium Gihtub](https://github.com/VSCodium/vscodium)

### 克隆项目

如果您想要贡献代码到本项目，您需要首先将本项目 fork 到自己的账号，然后拉取自己账号的仓库，克隆后提交修改推送，最后pr到本项目，并且由管理员审核后才能通过。


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


# 📝最后的话

再次感谢您的理解与宝贵贡献！