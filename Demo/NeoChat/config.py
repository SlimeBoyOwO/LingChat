import os
from dotenv import load_dotenv
load_dotenv()

# AI配置
API_KEY = os.getenv("API_KEY")
API_URL = "https://api.deepseek.com/chat/completions"
MODEL_NAME = "deepseek-chat"
AI_NAME = os.getenv("AI_NAME")
MAX_TOKENS = 4096                         # DeepSeek API允许的最大输出Token数，根据模型调整
TEMPERATURE = 0.7                         # 生成文本的温度值，0.1-1.0，越高越随机。默认为0.7
API_TIMEOUT_SECONDS = 180                 # API请求的超时时间 (秒) (建议)

# 系统提示词(System Prompt)
SYSTEM_PROMPT = os.getenv("SYSTEM_PROMPT")

LLM_CONVERSATION_HISTORY_LIMIT = 50 # LLM对话历史长度限制，超过此长度将截断。根据模型和API限制调整。后续将改为RAG系统。
LLM_FREE_TIME_HISTORY_LIMIT = 15    # 在自由时间的“自动”对话顺序模式下，用于决策的LLM参考的最近历史记录数量。
AI_CHOICE_HISTORY_CONTEXT_COUNT = 8 # 在 AIChoice 模式下，系统判断AI决策时，额外参考的最近对话历史数量。

# 会话与历史记录
HISTORY_BASE_PATH = "data/Dialogue_history"    # 聊天记录保存路径 (这个在重构后可能不再直接使用，因为对话历史现在由save_manager管理并随存档保存)
CHROMA_DB_PATH = "data/chroma_db_store"      # RAG缓存路径，ChromaDB持久化存储路径。可安全删除，删除后会根据Json聊天记录重新生成，但更耗时。
SAVES_BASE_PATH = "data/saves"                 # 游戏存档保存路径
STORY_PACKS_BASE_PATH = "data/story_packs"     # 剧本包保存路径
CHARACTERS_BASE_PATH = "data/characters"       # AI角色人设保存路径
PLAYER_CHARACTERS_BASE_PATH = "data/player_characters" # 玩家人设包保存路径
USER_CONFIG_PATH = "data/user_config.py" # user.py的路径

# 调试与日志
DEBUG_MODE = False                         #开启/关闭开发者模式。设置为True/False将开关Debug日志。

# RAG (Retrieval Augmented Generation) 设置
USE_RAG = True                            # 是否启用RAG功能
MAX_CONTEXT_MESSAGES_SLIDING_WINDOW = 20  # 限制本轮次对话最近发送的上下文消息数量，以防token爆炸
RAG_RETRIEVAL_COUNT = 3                   # 配置RAG搜索多少条聊天记录
RAG_CONTEXT_M_BEFORE = 2                  # 把检索到的聊天记录之前的m条消息发送给llm
RAG_CONTEXT_N_AFTER = 2                   # 把检索到的聊天记录之后的n条消息发送给llm
RAG_CANDIDATE_MULTIPLIER = 3              # 代码稳定性设置。为获取RAG_RETRIEVAL_COUNT个块，实际从ChromaDB查询的候选倍数，不建议改动

RAG_PROMPT_PREFIX = (                     # RAG 内容的前缀提示
    "--- 以下是根据你的历史记忆检索到的相关对话片段，请参考它们来回答当前问题。这些是历史信息，不是当前对话的一部分： ---"
)
RAG_PROMPT_SUFFIX = (
    "--- 以上是历史记忆检索到的内容。请注意，这些内容用于提供背景信息，你不需要直接回应它们，而是基于它们和下面的当前对话来生成回复。 ---"
)

# SentenceTransformer 嵌入模型进度条设置。如果你希望在DEBUG_MODE=False时也显示SentenceTransformer的进度条，可以设置为True
SHOW_EMBEDDING_PROGRESS = DEBUG_MODE