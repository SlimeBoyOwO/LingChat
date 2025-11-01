# External Embedding Service for RAG

这是一个为 `LingChat` 项目中的RAG（Retrieval-Augmented Generation）系统提供文本向量化功能的独立微服务。

## 🎯 目的

取消`LingChat`主项目的torch依赖，使用HTTP请求外部torch模块来精简依赖。

在后续，这个项目会在release提供编译的版本，或者使用rust重写以体高性能。



## 🚀 使用说明 (Getting Started)

按照以下步骤即可在本地启动并运行嵌入服务。

### 步骤 1: 环境准备与安装依赖

打开终端，进入该目录，然后运行以下命令：

```bash
pip install -r requirements.txt
```

### 步骤 2: 下载嵌入模型

在启动服务之前，需要先将预训练的嵌入模型下载到本地。

运行 `download_model.py` 脚本：

```bash
python download_model.py
```

脚本会自动从Hugging Face Hub下载 `all-MiniLM-L6-v2` 模型，并将其保存在一个名为 `embedding_models` 的子目录中。成功后，您会看到类似以下的输出：

```
INFO: ... 模型 'all-MiniLM-L6-v2' 似乎已存在于: /path/to/your/project/embedding_models/all-MiniLM-L6-v2
INFO: 跳过下载。如果需要重新下载，请先删除该目录。
```
或（首次运行时）：
```
INFO: ... 开始下载模型: 'all-MiniLM-L6-v2'
...
INFO: ==================================================
INFO: 模型已成功下载并保存！
INFO: 路径: /path/to/your/project/embedding_models/all-MiniLM-L6-v2
INFO: ==================================================
```

### 步骤 3: 启动嵌入服务

模型准备就绪后，启动Flask Web服务。

运行 `embedding_service.py` 脚本：
```bash
python embedding_service.py
```

如果一切正常，服务将启动并监听 `5001` 端口。您会看到如下日志信息：
```
INFO: ... 正在从本地路径加载模型: /path/to/your/project/embedding_models/all-MiniLM-L6-v2
INFO: ... 模型 'all-MiniLM-L6-v2' 加载成功，运行在设备: 'cpu' (或 'cuda')
INFO: ... 嵌入模型服务已启动，监听地址 http://0.0.0.0:5001
```

**至此，嵌入服务已成功运行！** 主项目的`RAGSystem`现在可以通过 `http://localhost:5001/encode` 来获取文本的嵌入向量了。


## 👨‍💻 开发者须知

### API 端点

本服务提供以下两个API端点：

1.  **`GET /health`**
    -   **目的**: 检查服务健康状况和模型加载状态。
    -   **成功响应 (200 OK)**: `{"status": "ok", "model_loaded": true}`
    -   **失败响应 (503 Service Unavailable)**: `{"status": "error", "model_loaded": false}`

2.  **`POST /encode`**
    -   **目的**: 将一批文本转换为嵌入向量。
    -   **请求体 (JSON)**:
        ```json
        {
          "texts": ["文本1", "文本2", ...]
        }
        ```
    -   **成功响应 (200 OK)**:
        ```json
        {
          "embeddings": [
            [向量1],
            [向量2],
            ...
          ]
        }
        ```
    -   **错误响应 (400/500)**: 返回包含错误信息的JSON对象，如 `{"error": "请求体中必须包含一个名为 'texts' 的非空列表"}`。

### 配置

如果您需要修改默认配置，可以直接编辑 `embedding_service.py` 和 `download_model.py` 文件顶部的常量：

-   `EMBEDDING_MODEL_NAME`: 要使用的Sentence Transformer模型名称。**注意：两个文件中的此名称必须保持一致！**
-   `MODEL_PATH` / `SAVE_DIRECTORY`: 模型在本地的存储路径。
-   `SERVER_PORT`: 服务监听的端口号。如果修改此端口，请确保主项目中 `RAGSystem` 的 `RAG_EMBEDDING_SERVICE_URL` 配置也同步更新。