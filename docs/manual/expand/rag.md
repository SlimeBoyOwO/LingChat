---
title: RAG 系统使用说明
description: 了解LingChat的RAG系统，包括功能概述、启用配置、依赖安装和使用方法，提升AI对话的连贯性。
---

# 🔍 RAG 系统使用说明

本项目集成了基于检索增强生成（Retrieval-Augmented Generation，RAG）的上下文增强系统，可以显著提升 AI 对话的连贯性和上下文理解能力。

## 功能概述

RAG 系统会自动保存对话历史，并在用户提问时：

1. 检索与当前问题最相关的历史对话片段
2. 将这些相关片段以上下文形式提供给大语言模型
3. 增强模型生成更加连贯、符合上下文的回答

## 启用与配置

默认情况下，RAG 系统处于禁用状态。若要启用，请在`.env`文件中添加以下配置：

```txt
USE_RAG=true
```

完整的配置参数包括：

```txt
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

## 依赖安装

RAG 系统需要安装额外的 Python 依赖：

```bash
# 使用 `uv` 安装
# uv pip install sentence-transformers chromadb torch
# 使用 `pip` 安装
pip install sentence-transformers chromadb torch
```

这些依赖已添加到项目的`requirements.txt`中，且默认安装。

## 工作原理

RAG 系统的工作流程如下：

```mermaid
graph TD
    A[用户提问] --> B{RAG系统启用?}
    B -- 否 --> C[直接处理问题]
    B -- 是 --> D[检索相关历史对话]
    D --> E[向量化处理]
    E --> F[ChromaDB索引检索]
    F --> G[选择最相关片段]
    G --> H[构建增强上下文]
    H --> I[生成回答]
    I --> J[返回用户]
```

1. **保存历史**：所有对话内容会被保存到独立的 JSON 文件中
2. **向量化**：使用 Sentence Transformers 将对话内容转换为向量嵌入
3. **索引存储**：使用 ChromaDB 存储和索引向量嵌入
4. **语义检索**：基于余弦相似度查找与当前查询最相关的历史片段
5. **上下文增强**：将相关历史片段添加到当前对话上下文中

## 独立存储

RAG 系统使用独立的存储机制，不会影响现有的对话历史管理系统：

- RAG 历史记录存储在`RAG_HISTORY_PATH`指定的路径中
- 向量数据库存储在`CHROMA_DB_PATH`指定的路径中

## 查看日志

RAG 系统会输出详细的日志，包括：

- 初始化过程
- 检索性能指标
- 找到的相关上下文信息

可以通过设置`LOG_LEVEL=DEBUG`来查看更详细的调试信息。

## 性能考虑

- 首次启动时，RAG 系统需要加载模型和历史数据，可能需要较长时间
- 在 CPU 环境下，向量化和检索过程可能较慢
- 如果可用，系统会自动使用 GPU 加速计算
