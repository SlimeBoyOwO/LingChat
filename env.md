# RAG系统环境变量配置

将以下环境变量添加到你的`.env`文件中，以启用和配置RAG系统。

```
# RAG系统基本配置
USE_RAG=true                           # 是否启用RAG系统，设置为true或false
RAG_HISTORY_PATH=rag_chat_history      # RAG历史记录存储路径
CHROMA_DB_PATH=chroma_db_store         # ChromaDB数据库存储路径

# RAG检索配置
RAG_RETRIEVAL_COUNT=3                  # 检索的相关上下文块数量
RAG_CANDIDATE_MULTIPLIER=3             # 候选检索结果的倍数
RAG_CONTEXT_M_BEFORE=2                 # 前向上下文窗口大小
RAG_CONTEXT_N_AFTER=2                  # 后向上下文窗口大小

# RAG提示配置
RAG_PROMPT_PREFIX="以下是根据你的问题从历史对话中检索到的相关片段，其中包含了对话发生的大致时间："  # RAG前缀提示
RAG_PROMPT_SUFFIX=""                   # RAG后缀提示（可选）
```

## 安装依赖

RAG系统需要安装以下依赖：

```bash
pip install sentence-transformers chromadb torch
```

请确保在启用RAG系统前已经安装这些依赖。如果未安装，系统会自动禁用RAG功能并显示警告信息。

## 目录说明

启用RAG系统后，系统会自动创建以下目录：

1. `RAG_HISTORY_PATH`指定的目录：用于存储RAG系统专用的聊天历史记录
2. `CHROMA_DB_PATH`指定的目录：用于存储ChromaDB向量数据库

这些目录与原有的聊天历史记录系统完全独立，不会互相影响。 