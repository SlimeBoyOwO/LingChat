# LingChat 帮助文档

## 常见问题解决方案

### 1. TTS服务问题
**问题**: TTS服务不可达，语音功能被禁用

**可能原因**:
- 防火墙阻止了端口访问
- simple-vits-api服务未运行
- 程序需要重新加载服务

**解决方案**:
1. 检查并开放防火墙相关端口
2. 确保simple-vits-api服务已启动
3. 重新启动LingChat程序

### 2. 大模型API请求错误

| 错误代码 | 问题描述 | 解决方案 |
|---------|---------|---------|
| 401 | API密钥无效 | 检查并更新LingChat设置中的API密钥 |
| 429 | 请求频率过高 | 降低请求频率或升级API套餐 |
| 500 | 服务器内部错误 | 稍后重试或联系服务提供商 |
| 503/504 | 服务不可用 | 检查API服务状态 |
| 400 | 无效请求或余额不足 | 检查请求格式或充值API账户 |
| 403 | 权限不足 | 检查API密钥权限 |
| 422 | 参数错误 | 检查所有必需参数 |
| 408 | 连接超时 | 检查网络连接 |

### 3. 端口占用问题
**问题**: "通常每个套接字地址只允许使用一次"

**解决方案**:
1. 请使用`netstat -ano`查找占用端口的进程
2. 终止相关进程或更改LingChat使用的端口
3. 重启计算机（终极解决方案）

### 4.ERROR: Could not find Python 3.10, 3.11, or 3.12 using the 'py' launcher.
**问题**: 找不到Python 3.10, 3.11或3.12
**解决方案**:
1. 确保已安装Python 3.10, 3.11或3.12,如未安装请前往下载！下载地址"[https://www.python.org/downloads/](https://www.python.org/downloads/)"
2. 在Windows上，确保Python安装路径已添加到系统环境变量中
3. 在Linux上，使用`update-alternatives --config python`设置默认Python版本
4. 在Mac上，使用`brew install python@3.10`安装Python 3.10

### 5.ERROR: Failed to activate the existing virtual environment. Check if it's corrupted.
**问题**: 无法激活现有虚拟环境，可能已损坏
**解决方案**:
1. 尝试重新创建虚拟环境
2. 检查虚拟环境目录的完整性
3. 确保使用正确的Python版本
4. 删除现有虚拟环境并重新创建
   ```bash
   rm -rf venv
   python -m venv venv
   source venv/bin/activate  # Linux/Mac
   venv\Scripts\activate  # Windows
   ```

### 6. ERROR: Failed to install the required packages.
**问题**: 无法安装所需的Python包
**解决方案**:
1. 检查网络连接
2. 确保使用正确的Python版本
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