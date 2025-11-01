# LingChat 帮助文档

## 常见问题解决方案

### 1. TTS 服务问题

**问题**: TTS 服务不可达，语音功能被禁用

**可能原因**:

- 防火墙阻止了端口访问
- simple-vits-api 服务未运行
- 程序需要重新加载服务

**解决方案**:

1. 检查并开放防火墙相关端口
2. 确保 simple-vits-api 服务已启动
3. 重新启动 LingChat 程序

### 2. 大模型 API 请求错误

| 错误代码 | 问题描述                      | 解决方案                                                |
| -------- | ----------------------------- | ------------------------------------------------------- |
| 400      | 无效请求或余额不足            | 检查请求格式或充值 API 账户                             |
| 401      | API 密钥无效                  | 检查并更新 LingChat 设置中的 API 密钥                   |
| 402      | 余额不足                      | 检查 API 账户余额或充值                                 |
| 403      | 权限不足                      | 检查 API 密钥权限                                       |
| 404      | 资源未找到                    | 检查 API 端点是否正确                                   |
| 405      | 方法不允许                    | 检查 API 文档或联系服务提供商                           |
| 406      | 不可接受的请求                | 检查请求头或联系开发团队                                |
| 407      | 请求超时                      | 检查网络连接或稍后重试                                  |
| 408      | 连接超时                      | 检查网络连接                                            |
| 409      | 请求冲突                      | 检查请求参数或联系服务提供商                            |
| 422      | 参数错误                      | 检查所有必需参数                                        |
| 429      | 请求频率过高/请求过多         | 降低请求频率或升级 API 套餐，减少请求频率或等待一段时间 |
| 500      | 服务器内部错误/内部服务器错误 | 稍后重试或联系服务提供商/联系 API 提供商                |
| 502      | 错误的网关                    | 检查 API 服务状态或稍后重试                             |
| 503      | 服务不可用                    | 检查 API 服务状态或稍后重试                             |
| 504      | 网关超时                      | 检查网络连接或稍后重试                                  |
| 505      | HTTP 版本不受支持             | 检查 API 文档或联系服务提供商                           |
| 511      | 需要网络认证                  | 检查网络连接                                            |
| 520      | 未知错误                      | 检查 API 服务状态或联系开发团队                         |

### 3. 端口占用问题

**问题**: "通常每个套接字地址只允许使用一次"

**解决方案**:

1. 请使用`netstat -ano`查找占用端口的进程
2. 终止相关进程或更改 LingChat 使用的端口
3. 重启计算机（终极解决方案）

### 4.ERROR: Could not find Python 3.10, 3.11, or 3.12 using the 'py' launcher.

**问题**: 找不到 Python 3.10, 3.11 或 3.12
**解决方案**:

1. 确保已安装 Python 3.10, 3.11 或 3.12,如未安装请前往下载！下载地址"[https://www.python.org/downloads/](https://www.python.org/downloads/)"
2. 在 Windows 上，确保 Python 安装路径已添加到系统环境变量中
3. 在 Linux 上，使用`update-alternatives --config python`设置默认 Python 版本
4. 在 Mac 上，使用`brew install python@3.10`安装 Python 3.10

### 5.ERROR: Failed to activate the existing virtual environment. Check if it's corrupted.

**问题**: 无法激活现有虚拟环境，可能已损坏
**解决方案**:

1. 尝试重新创建虚拟环境
2. 检查虚拟环境目录的完整性
3. 确保使用正确的 Python 版本
4. 删除现有虚拟环境并重新创建
   ```bash
   rm -rf venv
   python -m venv venv
   source venv/bin/activate  # Linux/Mac
   venv\Scripts\activate  # Windows
   ```

### 6. ERROR: Failed to install the required packages.

**问题**: 无法安装所需的 Python 包
**解决方案**:

1. 检查网络连接
2. 确保使用正确的 Python 版本
3. 尝试手动安装缺失的包
   ```bash
   pip install -r requirements.txt
   ```

### 7.ERROR: Failed to activate the new virtual environment.

**问题**: 无法激活新的虚拟环境
**解决方案**:

1. 确保虚拟环境已正确创建，文件地址在"./venv"
2. 检查虚拟环境目录是否存在
3. 尝试手动激活虚拟环境

```bash
source venv/bin/activate  # Linux/Mac
venv\Scripts\activate  # Windows
```

## 联系支持

如问题仍未解决，请加入联系开发团队提供以下信息：

- 错误日志截图
- 问题发生时的操作步骤
- 系统环境信息

## 联系方式

- Telegram Group : [aigalgame](https://t.me/aigalgame)
- QQ Group: [1055935861](https://qm.qq.com/cgi-bin/qm/qr?k=1055935861)
