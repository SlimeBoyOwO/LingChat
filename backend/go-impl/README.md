# LingChat Backend Golang Implementation

LingChat 的后端服务Golang实现

## 环境要求

- Go 1.21 或更高版本
- Docker 和 Docker Compose（可选，用于容器化部署）

## 本地开发环境搭建

### 1. 安装 Go

如果你还没有安装 Go，请按照以下步骤安装：

#### macOS (使用 Homebrew)
```bash
brew install go
```

#### Linux
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install golang-go

# CentOS/RHEL
sudo yum install golang
```

#### Windows

Windows请参考`go.dev`自行下载安装包，或使用`Winget`,`Scoop`等工具

### 2. 配置 Go 环境

确保设置以下环境变量（通常会自动配置，可通过`go env`检查）：

```bash
export GOPATH=$HOME/go
export PATH=$PATH:$GOPATH/bin
```

### 3. 克隆项目并安装依赖

```bash
# 进入项目目录
cd backend/go-impl

# 下载依赖
go mod download
```

### 4. 配置环境变量

复制 `.env.example` 文件（如果存在）或创建 `.env` 文件，并配置必要的环境变量：

```bash
cp .env.example .env  # 如果存在 .env.example
```

然后编辑 `.env` 文件，设置必要的配置项。

## 运行应用

### 本地运行

#### 方式一：直接运行源码
```bash
# 启动应用
go run cmd/app/main.go
```

#### 方式二：编译后运行可执行文件
```bash
# 编译应用（在当前目录生成可执行文件）
go build -o lingchat ./cmd/app

# 运行编译后的可执行文件
./lingchat
```

应用将在 `http://localhost:8765` 启动。

### 使用 Docker 运行

1. 构建 Docker 镜像：

```bash
docker build -t lingchat-backend .
```

2. 运行容器：

```bash
docker run -p 8765:8765 --env-file .env lingchat-backend
```

## 开发指南

### 项目结构

```
.
├── api/            # API 定义和接口
├── cmd/            # 主程序入口
├── internal/       # 内部实现
│   ├── clients/        # 各依赖服务的客户端实现
│   ├── config/         # 配置相关
│   └── service/        # 核心业务逻辑
├── go.mod          # Go 模块定义
├── go.sum          # 依赖校验
└── .env            # 环境配置
```

### 更新依赖

```bash
go mod tidy
```

## 测试

运行测试：

```bash
go test ./...
```

## 常见问题

1. 如果遇到端口被占用：
   - 检查是否有其他进程在使用 8765 端口
   - 可以在 `.env` 文件中修改端口配置

2. 如果遇到依赖问题：
   - 运行 `go mod tidy` 清理依赖
   - 确保所有依赖都正确安装
