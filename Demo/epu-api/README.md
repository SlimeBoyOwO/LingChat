# EPU-API (E Pluribus Unum - api)

这是一个简易但功能完备的API整合路由项目，允许通过统一接口访问多个主流AI模型服务，包括OpenAI、DeepSeek、Spark、Qwen和Ollama本地模型。

## 功能特点

1. **统一API端点**：提供`/v1/chat/completions`和`/chat/completions`两个兼容OpenAI的API端点
2. **多模型支持**：
   - OpenAI系列（GPT模型）
   - DeepSeek
   - Ollama本地模型
3. **灵活的API密钥管理**：
   - 支持多种API密钥传递方式
   - 配置文件中管理允许的API密钥
4. **安全日志**：
   - API密钥在日志中部分脱敏显示
   - 详细的请求处理日志
5. **模块化设计**：
   - 适配器模式实现模型切换
   - 易于扩展新模型支持

## 快速开始

### 安装依赖
```bash
pip install -r requirements.txt
```

### 配置
1. 复制`config.yaml.example`为`config.yaml`
2. 填写各平台API凭证：
### 运行服务
```bash
python app.py
```

服务将在`http://0.0.0.0:8090`启动

## API使用说明

### 认证方式
支持以下任一方式传递API密钥：
1. HTTP头 `X-API-KEY`
2. HTTP头 `Authorization: Bearer <key>`
3. 查询参数 `?apikey=<key>`
4. JSON请求体中的 `apikey` 字段

### 请求示例
```bash
curl http://localhost:8090/v1/chat/completions \
  -H "X-API-KEY: your-api-key-1" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4.1",
    "messages": [{"role": "user", "content": "你好，介绍一下你自己"}]
  }'
```


## 支持的模型列表

| 模型前缀 | 对应服务 | 配置文件节点 | 适配器文件 |
|----------|----------|--------------|------------|
| `gpt-`   | OpenAI   | `openai`     | `openai_adapter.py` |
| `deepseek-` | DeepSeek | `deepseek`   | `deepseek_adapter.py` |
| `ollama-` | Ollama本地 | `ollama`    | `ollama_adapter.py` |

## 注意事项

1. **Ollama本地模型**：
   - 使用前需先[安装Ollama](https://ollama.com/)
   - 启动Ollama服务：`ollama serve`
   - 下载所需模型：`ollama pull <model-name>`
   - 调用时使用`ollama-`前缀，如`ollama-llama3`

3. **日志说明**：
   - API密钥在日志中会显示为首4位+星号+尾4位
   - 完整日志包含请求处理时间和模型信息

4. **Spark适配器**：
   - 需要配置`app_id`、`api_key`和`api_secret`
   - 使用WebSocket协议进行通信

## 项目结构

```
.
├── app.py                 # 主应用入口
├── config.yaml            # 配置文件
├── requirements.txt       # 依赖清单
└── adapters/              # 模型适配器
    ├── __init__.py        # 适配器选择器
    ├── deepseek_adapter.py
    ├── ollama_adapter.py
    ├── openai_adapter.py
    ├── qwen_adapter.py
    └── spark_adapter.py
```

## 扩展新模型

1. 在`adapters/`目录创建新适配器，实现`create_chat_completion`方法
2. 在`adapters/__init__.py`的`MODEL_MAPPING`中添加模型前缀映射
3. 在`config.yaml`中添加对应的配置节点

## 健康检查
```
GET http://localhost:8090
```

响应：
```json
{"status": "ok", "message": "API is running"}
```