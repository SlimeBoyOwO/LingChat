import sys
import os
import json
import uuid
import re
import torch
from datetime import datetime
from typing import List, Dict, Tuple, Optional, Any
from .logger import logger, TermColors

_sentence_transformer_imported_ok = True
_chromadb_imported_ok = True

try:
    from sentence_transformers import SentenceTransformer
except ImportError:
    _sentence_transformer_imported_ok = False
    # Placeholder class if sentence-transformers is not found
    class SentenceTransformer:
        def __init__(self, *args, **kwargs): pass
        def encode(self, *args, **kwargs): raise NotImplementedError("SentenceTransformer is not available.")

try:
    import chromadb
except ImportError:
    _chromadb_imported_ok = False
    # Placeholder class if chromadb is not found
    class chromadb:
        class PersistentClient:
            def __init__(self, *args, **kwargs): pass
            def get_or_create_collection(self, *args, **kwargs): raise NotImplementedError("chromadb is not available.")
        def get_collection(self, *args, **kwargs): raise NotImplementedError("chromadb is not available.")


class RAGSystem:
    '''
    RAG (Retrieval-Augmented Generation) 系统，该系统用于检索与当前用户查询相关的历史对话片段，
    并将这些片段作为上下文提供给大语言模型，从而增强模型生成回复的相关性和准确性，赋予模型长期记忆的能力。

    调用:
    1. 实例化 `RAGSystem(config)`。
    2. 调用 `initialize()` 方法来加载模型和数据库。
    3. 在处理用户输入时，调用 `prepare_rag_messages(user_input)` 获取上下文。
    4. 在会话结束时，调用 `add_session_to_history(session_messages)` 将对话存入历史记录。
    '''
    def __init__(self, config, character_id: int):
        '''
        初始化RAG系统实例，现在与特定角色绑定。
        :param config: RAG的通用配置
        :param character_id: 当前AI角色的ID
        '''
        if not character_id:
            raise ValueError("RAGSystem必须使用一个有效的character_id进行初始化。")

        self.config = config
        self.character_id = character_id
        self.embedding_model = None
        self.chroma_client = None
        self.chroma_collection = None
        self.historical_sessions_map = {}
        self.flat_historical_messages = []
        
        # 动态生成集合名称，基于角色ID
        self.CHROMA_COLLECTION_NAME = f"rag_collection_char_{self.character_id}"
        self.EMBEDDING_MODEL_NAME = 'all-MiniLM-L6-v2'

    def initialize(self) -> bool:
        '''
        初始化RAG系统的所有核心组件。加载嵌入模型（SentenceTransformer）并连接到ChromaDB向量数据库。

        调用:
        在应用启动时，创建 `RAGSystem` 实例后应立即调用此方法。

        返回:
        - bool: 如果所有组件都成功初始化，则返回 `True`，否则返回 `False`。

        注意事项:
        - 如果配置中 `USE_RAG` 为 `False`，此方法将直接返回 `False` 并跳过所有初始化步骤。
        - 如果依赖的 `sentence-transformers` 或 `chromadb` 库未安装，将记录错误并初始化失败。
        '''
        if not getattr(self.config, 'USE_RAG', False):
            logger.info("RAG功能已禁用 (根据配置)。")
            return False

        if not _sentence_transformer_imported_ok:
            logger.error(f"RAG组件初始化失败: 'sentence-transformers' 模块未找到。请运行: pip install sentence-transformers")
            return False
        if not _chromadb_imported_ok:
            logger.error(f"RAG组件初始化失败: 'chromadb' 模块未找到。请运行: pip install chromadb")
            return False

        logger.debug("开始初始化RAG组件...")
        try:
            # Construct the local model path relative to the current file (RAG.py)
            # RAG.py is in 'backend/core/'
            # Model should be in 'backend/core/memory_rag/models/all-MiniLM-L6-v2'
            script_dir = os.path.dirname(os.path.abspath(__file__))
            local_model_path = os.path.join(script_dir, 'memory_rag', 'models', self.EMBEDDING_MODEL_NAME)

            logger.info(f"RAG: 准备从本地路径加载嵌入模型: {local_model_path}")

            if not os.path.isdir(local_model_path):
                logger.error(f"本地模型路径不存在或不是文件夹: {local_model_path}")
                logger.error("请确保模型已下载到 'backend/core/memory_rag/models' 目录下。")
                logger.error("您可以运行 'backend/core/memory_rag/downloading.py' 脚本来下载模型。")
                return False

            logger.debug(f"RAG: 初始化Sentence Transformer模型...")
            device = 'cuda' if torch.cuda.is_available() else 'cpu'
            self.embedding_model = SentenceTransformer(local_model_path, device=device)
            logger.debug(f"RAG: 本地Sentence Transformer模型 ({self.EMBEDDING_MODEL_NAME}) 加载成功。当前使用 {device} 进行RAG向量库匹配的推理。")

            chroma_db_path = getattr(self.config, 'CHROMA_DB_PATH', './chroma_db_store')
            logger.debug(f"RAG: 初始化ChromaDB客户端 (记忆库将存储在 '{chroma_db_path}').")
            self.chroma_client = chromadb.PersistentClient(path=chroma_db_path)
            logger.debug(f"RAG: ChromaDB客户端初始化成功 (数据路径: {chroma_db_path})。")

            logger.debug(f"RAG: 获取或创建ChromaDB集合: {self.CHROMA_COLLECTION_NAME}")
            self.chroma_collection = self.chroma_client.get_or_create_collection(
                name=self.CHROMA_COLLECTION_NAME,
                metadata={"hnsw:space": "cosine"}
            )
            logger.info(f"RAG: 角色 {self.character_id} 的ChromaDB集合 '{self.CHROMA_COLLECTION_NAME}' 已就绪。当前包含 {self.chroma_collection.count()} 条目。")
            
            self.load_historical_data()
            
            return True
        except Exception as e:
            logger.error(f"RAG组件初始化过程中发生错误: {e}")
            logger.debug(f"RAG Initialization Error during component setup: {e}", exc_info=True)
            self.embedding_model = None
            self.chroma_client = None
            self.chroma_collection = None
            return False

    def load_historical_data(self) -> Tuple[int, int]:
        '''
        从指定目录加载历史对话JSON文件并建立索引。用于在系统启动时填充RAG系统的知识库，使其能够检索过去的对话。

        调用:
        通常由 `initialize()` 方法自动调用。

        返回:
        - Tuple[int, int]: 一个元组，包含成功加载的会话文件数量和消息总数。

        注意事项:
        - 如果历史记录路径不存在，会自动创建该目录。
        - 会跳过格式错误或无法解析的JSON文件。
        - 加载数据后，会自动调用 `add_messages_to_index` 将消息内容存入向量数据库。
        '''
        if not getattr(self.config, 'USE_RAG', False) or not self.chroma_collection:
            logger.debug("RAG: 组件未初始化或RAG已禁用，跳过历史数据加载。")
            return 0, 0
            
        # 为每个角色创建独立的历史记录目录
        base_path = getattr(self.config, 'RAG_HISTORY_PATH', './rag_chat_history')
        history_path = os.path.join(base_path, f"character_{self.character_id}")
        if not os.path.exists(history_path):
            logger.info(f"RAG: 角色 {self.character_id} 的历史对话路径不存在: {history_path}，将创建该目录。")
            os.makedirs(history_path, exist_ok=True)
            return 0, 0
            
        logger.debug(f"RAG: 开始从 {history_path} 加载历史对话数据...")
        
        all_messages_flat = []
        historical_sessions_map = {}
        
        loaded_files_count = 0
        total_messages_loaded = 0
        
        for root, _, files in os.walk(history_path):
            sorted_files = sorted([f for f in files if f.endswith(".json")])
            for filename in sorted_files:
                filepath = os.path.join(root, filename)
                session_time_str = self._parse_session_time_from_filename(filename)
                try:
                    with open(filepath, 'r', encoding='utf-8') as f:
                        session_data = json.load(f)
                        if isinstance(session_data, list) and session_data:
                            historical_sessions_map[filename] = []
                            for idx, msg in enumerate(session_data):
                                msg_copy_flat = msg.copy()
                                msg_copy_flat['_source_file'] = filename
                                msg_copy_flat['_original_idx'] = idx
                                msg_copy_flat['_session_timestamp_str'] = session_time_str
                                all_messages_flat.append(msg_copy_flat)

                                msg_copy_map = msg.copy()
                                msg_copy_map['_source_file'] = filename
                                msg_copy_map['_original_idx'] = idx
                                msg_copy_map['_session_timestamp_str'] = session_time_str
                                historical_sessions_map[filename].append(msg_copy_map)
                            loaded_files_count += 1
                            total_messages_loaded += len(session_data)
                except (json.JSONDecodeError, IOError) as e:
                    logger.warning(f"RAG: 加载历史文件 {filepath} 失败: {e}")
                    logger.debug(f"RAG: Failed to load history file {filepath}: {e}", exc_info=True)

        if loaded_files_count > 0:
            logger.debug(f"RAG: 成功从 {loaded_files_count} 个文件中加载了 {total_messages_loaded} 条历史消息。")
        else:
            logger.warning("RAG: 未找到或加载任何有效的历史会话文件。")
            
        logger.debug(f"RAG: 共映射 {len(historical_sessions_map)} 个会话。")
        
        self.flat_historical_messages = all_messages_flat
        self.historical_sessions_map = historical_sessions_map
        
        if all_messages_flat:
            self.add_messages_to_index(all_messages_flat)
            
        return loaded_files_count, total_messages_loaded

    def _parse_session_time_from_filename(self, filename: str) -> str:
        '''
        从会话历史文件名中解析出格式化的时间字符串。用于从如 `session_20231027_143000.json` 的文件名中提取易于阅读的时间戳。
        '''
        match = re.search(r"session_(\d{8}_\d{6})\.json", filename)
        if match:
            try:
                dt_obj = datetime.strptime(match.group(1), "%Y%m%d_%H%M%S")
                return dt_obj.strftime("%Y年%m月%d日 %H:%M")
            except ValueError:
                logger.debug(f"RAG: 无法从文件名 {filename} 解析有效日期时间。")
                return "未知时间"
        return "未知时间"

    def add_messages_to_index(self, messages_with_metadata: List[Dict]) -> bool:
        '''
        将一批消息添加到向量数据库索引中。为消息内容生成向量嵌入，并将其与元数据一同存入ChromaDB，以便后续检索。

        调用:
        - `load_historical_data` 调用此方法来索引存量的历史数据。
        - `add_session_to_history` 调用此方法来索引新的会话数据。
        '''
        if not getattr(self.config, 'USE_RAG', False) or not self.embedding_model or not self.chroma_collection:
            logger.debug("RAG: 组件未初始化或RAG已禁用，跳过索引。")
            return False

        if not messages_with_metadata:
            logger.info("RAG: 无消息可供索引。")
            return False

        logger.debug(f"RAG: 准备为 {len(messages_with_metadata)} 条消息建立索引...")
        documents, metadatas, ids = [], [], []

        for msg in messages_with_metadata:
            content, role = msg.get('content'), msg.get('role')
            source_file, original_idx = msg.get('_source_file'), msg.get('_original_idx')

            if not all([content, isinstance(content, str), role, source_file is not None, original_idx is not None]):
                logger.debug(f"RAG: 跳过无效消息进行索引 (字段缺失): {str(msg)[:100]}...")
                continue

            message_id_str = f"{source_file}_{original_idx}_{role}_{content[:100]}"
            message_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, message_id_str))
            documents.append(content)
            metadatas.append({"role": role, "source_file": source_file, "original_idx": original_idx})
            ids.append(message_id)

        if not documents:
            logger.warning("RAG: 筛选后无有效文档可供索引。")
            return False

        logger.debug(f"RAG: 正在为 {len(documents)} 个文档生成嵌入向量...")
        embeddings = self.embedding_model.encode(documents).tolist()
        logger.debug(f"RAG: 嵌入向量生成完毕。Shape: ({len(embeddings)}, {len(embeddings[0]) if embeddings else 0})")

        try:
            batch_size = 500
            for i in range(0, len(ids), batch_size):
                batch_ids = ids[i:i + batch_size]
                batch_embeddings = embeddings[i:i + batch_size]
                batch_documents = documents[i:i + batch_size]
                batch_metadatas = metadatas[i:i + batch_size]
                self.chroma_collection.upsert(
                    ids=batch_ids, 
                    embeddings=batch_embeddings, 
                    documents=batch_documents,
                    metadatas=batch_metadatas
                )
                logger.debug(f"RAG: Upserted batch {i // batch_size + 1} ({len(batch_ids)} documents).")
            logger.debug(f"RAG: 成功向ChromaDB中添加/更新了 {len(ids)} 个文档。")
            logger.debug(f"RAG: 索引库 '{self.CHROMA_COLLECTION_NAME}' 当前总条目: {self.chroma_collection.count()}")
            return True
        except Exception as e:
            logger.error(f"RAG: 向ChromaDB中Upsert文档时出错: {e}")
            logger.debug(f"RAG: ChromaDB Upsert Error: {e}", exc_info=True)
            return False

    def add_session_to_history(self, session_messages: List[Dict], session_filepath: Optional[str] = None) -> Optional[str]:
        '''
        将一次完整的对话会话保存到历史记录文件，并更新RAG索引。

        调用:
        在对话流程结束时调用，传入该次会话的所有消息。

        注意事项:
        - 会自动过滤掉 `role` 为 `system` 的消息，因为它们通常不包含对话内容。
        - 如果 `session_messages` 包含来自多个不同会话ID的消息，它会尝试找出主会话并只保存该会话的消息。
        '''
        if not getattr(self.config, 'USE_RAG', False):
            logger.debug("RAG: RAG功能已禁用，跳过会话保存。")
            return None
            
        if not session_messages:
            logger.warning("RAG: 无有效会话消息可供保存。")
            return None
        
        filtered_messages = []
        if len(session_messages) > 0 and 'session_id' in session_messages[0]:
            session_ids = [msg.get('session_id') for msg in session_messages if 'session_id' in msg]
            main_session_id = max(set(session_ids), key=session_ids.count) if session_ids else None
            
            if main_session_id:
                filtered_messages = [msg for msg in session_messages if msg.get('session_id') == main_session_id]
            
        if not filtered_messages:
            filtered_messages = session_messages
            
        if len(filtered_messages) < len(session_messages) and logger.should_print_context():
            logger.debug(f"RAG: 根据session_id过滤后，消息数量从 {len(session_messages)} 减少到 {len(filtered_messages)}")
            
        filtered_messages = [msg for msg in filtered_messages if msg.get('role') != 'system']
        
        if not filtered_messages:
            logger.warning("RAG: 过滤后无有效会话消息可供保存。")
            return None
            
        try:
            if not session_filepath:
                session_filepath = self.get_history_filepath()
                
            os.makedirs(os.path.dirname(session_filepath), exist_ok=True)
            
            with open(session_filepath, 'w', encoding='utf-8') as f:
                json.dump(filtered_messages, f, ensure_ascii=False, indent=4)
            logger.debug(f"RAG: 会话历史已保存到: {session_filepath}")
            
            filename = os.path.basename(session_filepath)
            session_time_str = self._parse_session_time_from_filename(filename)
            
            messages_with_metadata = []
            for idx, msg in enumerate(filtered_messages):
                msg_copy = msg.copy()
                msg_copy['_source_file'] = filename
                msg_copy['_original_idx'] = idx
                msg_copy['_session_timestamp_str'] = session_time_str
                messages_with_metadata.append(msg_copy)
                
            self.historical_sessions_map[filename] = messages_with_metadata
            self.flat_historical_messages.extend(messages_with_metadata)
            
            self.add_messages_to_index(messages_with_metadata)
            
            logger.debug(f"RAG: 保存了 {len(filtered_messages)} 条消息 (过滤前: {len(session_messages)})")
            
            return session_filepath
        except Exception as e:
            logger.error(f"RAG: 保存会话历史失败: {e}")
            logger.debug(f"RAG: Session history save error: {e}", exc_info=True)
            return None

    def get_history_filepath(self) -> str:
        '''
        为新的会话记录创建一个有组织的、基于日期的文件路径，现在按角色分目录存储。

        返回:
        - str: 一个完整的文件路径，例如 `.../rag_chat_history/character_1/2023年10月/27日/session_...json`。
        注意事项:
        此方法会自动创建尚不存在的角色/年/月/日目录结构。
        '''
        now = datetime.now()
        base_path = getattr(self.config, 'RAG_HISTORY_PATH', './rag_chat_history')
        character_specific_path = os.path.join(base_path, f"character_{self.character_id}")
        year_month_path = os.path.join(character_specific_path, now.strftime("%Y年%m月"))
        day_path = os.path.join(year_month_path, now.strftime("%d日"))
        os.makedirs(day_path, exist_ok=True)
        session_start_time_str = now.strftime("%Y%m%d_%H%M%S")
        return os.path.join(day_path, f"session_{session_start_time_str}.json")

    def get_relevant_messages(self, query_text: str) -> List[Dict]:
        '''
        这是RAG系统的核心检索功能。它找到与用户问题最相似的历史对话点，并提取
        该点周围的对话，以形成一个连贯的上下文片段。

        参数:
        - query_text: 用户的输入或其他用于检索的查询字符串。

        返回:
        - List[Dict]: 一个消息字典列表，包含了所有检索到的相关上下文信息。如果未找到
          相关信息，则返回空列表。

        注意事项:
        - 检索出的消息内容会被修改，以包含来源会话的时间戳，例如 `[历史对话片段 - 时间] ...`。
        - 该方法会进行去重，避免同一个上下文片段被多次添加。
        - 检索参数（如检索数量、上下文窗口大小）由配置文件中的 `RAG_*` 选项控制。
        '''
        if not getattr(self.config, 'USE_RAG', False) or not self.embedding_model or not self.chroma_collection:
            logger.warning("RAG: 组件未初始化或RAG已禁用，跳过检索。")
            return []
        if not query_text:
            logger.warning("RAG: 查询文本为空，跳过RAG检索。")
            return []
        if self.chroma_collection.count() == 0:
            logger.warning("RAG: ChromaDB集合为空，跳过RAG检索。")
            return []

        start_time = datetime.now()
        
        retrieval_count = getattr(self.config, 'RAG_RETRIEVAL_COUNT', 3)
        candidate_multiplier = getattr(self.config, 'RAG_CANDIDATE_MULTIPLIER', 3)
        context_before = getattr(self.config, 'RAG_CONTEXT_M_BEFORE', 2)
        context_after = getattr(self.config, 'RAG_CONTEXT_N_AFTER', 2)
        
        num_candidates_to_fetch = retrieval_count * candidate_multiplier
        num_candidates_to_fetch = min(num_candidates_to_fetch, self.chroma_collection.count())

        logger.info_color(f"RAG: 正在为查询 \"{query_text[:50]}...\" 检索最多 {num_candidates_to_fetch} 个候选片段...",
                   TermColors.BLUE)
        
        device = 'cuda' if torch.cuda.is_available() else 'cpu'
        logger.debug(f"RAG: 当前检索设备: {device}, 嵌入模型: {self.EMBEDDING_MODEL_NAME}")
        logger.debug(f"RAG: 检索参数: 目标块数={retrieval_count}, 候选倍数={candidate_multiplier}, 上文窗口={context_before}, 下文窗口={context_after}")
                   
        vector_start_time = datetime.now()
        query_embedding = self.embedding_model.encode([query_text], show_progress_bar=False)[0].tolist()
        vector_end_time = datetime.now()
        vector_time_ms = (vector_end_time - vector_start_time).total_seconds() * 1000
        logger.debug(f"RAG: 查询向量计算耗时: {vector_time_ms:.2f}ms")

        try:
            db_start_time = datetime.now()
            results = self.chroma_collection.query(
                query_embeddings=[query_embedding],
                n_results=num_candidates_to_fetch,
                include=["documents", "metadatas", "distances"]
            )
            db_end_time = datetime.now()
            db_time_ms = (db_end_time - db_start_time).total_seconds() * 1000
            logger.debug(f"RAG: ChromaDB查询耗时: {db_time_ms:.2f}ms")
            
        except Exception as e:
            logger.error(f"RAG 查询ChromaDB失败: {e}")
            logger.debug(f"RAG: ChromaDB Query Error: {e}", exc_info=True)
            return []

        final_rag_messages, used_chroma_doc_ids, added_message_contents_to_llm = [], set(), set()
        retrieved_blocks_count = 0

        if results and results.get('ids') and results['ids'][0]:
            logger.debug(f"RAG: ChromaDB返回 {len(results['ids'][0])} 个候选结果。")
            
            if logger.should_print_context():
                logger.debug("\n------ RAG候选结果详情 ------")
                for i in range(min(5, len(results['ids'][0]))):
                    try:
                        doc_id = results['ids'][0][i]
                        distance = results['distances'][0][i]
                        content = results['documents'][0][i][:100] + "..." if len(results['documents'][0][i]) > 100 else results['documents'][0][i]
                        metadata = results['metadatas'][0][i]
                        logger.debug(f"候选[{i+1}]: 距离={distance:.4f}, 角色={metadata.get('role', 'unknown')}, ID={doc_id}")
                        logger.debug(f"    内容: {content}")
                    except Exception as e:
                        logger.debug(f"    无法显示候选[{i+1}]: {e}")
                if len(results['ids'][0]) > 5:
                    logger.debug(f"    ... 及其他 {len(results['ids'][0])-5} 个候选")
                logger.debug("------ 候选结果结束 ------\n")
            
            for i in range(len(results['ids'][0])):
                if retrieved_blocks_count >= retrieval_count:
                    logger.debug(f"RAG: 已达到期望的 {retrieval_count} 个独立上下文块。停止处理候选。")
                    break
                try:
                    core_doc_id, core_doc_content = results['ids'][0][i], results['documents'][0][i]
                    metadata, distance = results['metadatas'][0][i], results['distances'][0][i]
                except (IndexError, TypeError, KeyError) as e:
                    logger.warning(f"RAG: ChromaDB结果索引 {i} 处数据不完整或格式错误。跳过。详细: {e}")
                    continue

                if core_doc_id in used_chroma_doc_ids or core_doc_content == query_text:
                    logger.debug(f"RAG: 跳过已使用或与查询相同的文档 ID {core_doc_id}.")
                    continue

                source_file, original_idx = metadata.get("source_file"), metadata.get("original_idx")
                if source_file not in self.historical_sessions_map or not isinstance(original_idx, int):
                    logger.warning(f"RAG: 文档 {core_doc_id} 元数据不完整或会话未在Map中找到。跳过。")
                    continue

                current_session_messages = self.historical_sessions_map[source_file]
                if not (0 <= original_idx < len(current_session_messages)):
                    logger.warning(f"RAG: 原始索引 {original_idx} 超出 '{source_file}' 会话边界。跳过。")
                    continue

                start_idx = max(0, original_idx - context_before)
                end_idx = min(len(current_session_messages), original_idx + context_after + 1)

                context_block_for_llm, context_block_display_info, potential_block_messages = [], [], []
                for j in range(start_idx, end_idx):
                    msg_obj = current_session_messages[j]
                    msg_content, msg_role = msg_obj.get("content"), msg_obj.get("role", "unknown")
                    session_time_str = msg_obj.get("_session_timestamp_str", "未知时间")

                    if msg_content and msg_content not in added_message_contents_to_llm:
                        contextualized_content = f"[历史对话片段] {msg_content}"
                        potential_block_messages.append({"role": msg_role, "content": contextualized_content})
                        is_core = " (核心检索)" if j == original_idx else ""
                        context_block_display_info.append(
                            f"  - ({session_time_str}) [{msg_role}]{is_core}: \"{msg_content[:50]}...\"")

                if potential_block_messages:
                    context_block_for_llm.extend(potential_block_messages)
                    for msg in potential_block_messages: 
                        added_message_contents_to_llm.add(msg['content'])
                    used_chroma_doc_ids.add(core_doc_id)
                    retrieved_blocks_count += 1
                    final_rag_messages.extend(context_block_for_llm)
                    logger.info_color(
                        f"\nRAG 系统检索到历史对话片段 (核心距离: {distance:.4f}, 源: {source_file}, 核心索引: {original_idx}):",
                        TermColors.MAGENTA)
                    for line in context_block_display_info:
                        logger.info_color(f"{line}", TermColors.MAGENTA)
                    logger.debug(f"RAG: 添加上下文块 (ID {core_doc_id}). LLM的RAG消息总数: {len(final_rag_messages)}")
                    
                    if logger.should_print_context():
                        block_chars = sum(len(msg.get('content', '')) for msg in potential_block_messages)
                        msg_types = {}
                        for msg in potential_block_messages:
                            role = msg.get('role', 'unknown')
                            msg_types[role] = msg_types.get(role, 0) + 1
                        role_info = ", ".join([f"{r}:{c}" for r, c in msg_types.items()])
                        logger.debug(f"RAG: 上下文块{retrieved_blocks_count}包含 {len(potential_block_messages)} 条消息 ({role_info}), 共 {block_chars} 字符")
                else:
                    logger.debug(f"RAG: 核心文档ID {core_doc_id} 的上下文块为空或所有消息已去重。")

        end_time = datetime.now()
        total_time_ms = (end_time - start_time).total_seconds() * 1000
        
        if not final_rag_messages:
            logger.info_color("RAG 系统: 未在历史记录中找到与当前问题相关的、非重复的消息。", TermColors.YELLOW)
        else:
            total_chars = sum(len(msg.get('content', '')) for msg in final_rag_messages)
            logger.info_color(
                f"RAG: 为LLM准备了 {len(final_rag_messages)} 条消息，来自 {retrieved_blocks_count} 个不同的RAG上下文块。",
                TermColors.GREEN)
            logger.debug(f"RAG: 检索时间: {total_time_ms:.2f}ms, 平均每块: {total_time_ms/max(1, retrieved_blocks_count):.2f}ms, 总字符数: {total_chars}")
            
        return final_rag_messages

    def prepare_rag_messages(self, user_input: str) -> List[Dict]:
        '''
        为用户输入准备完整的RAG上下文消息列表。
        
        注意事项:
        - 此方法会处理用户界面的加载动画，并在日志中提供详细的执行统计信息。
        - 会自动过滤掉检索结果中可能存在的`system`角色的消息，以避免污染主提示。
        '''
        if not getattr(self.config, 'USE_RAG', False):
            logger.debug("RAG: RAG功能已禁用，跳过检索准备。")
            return []
            
        if logger.should_print_context():
            logger.debug(f"\n------ RAG准备阶段 ------")
            logger.debug(f"RAG: 开始为用户输入准备RAG上下文，输入长度: {len(user_input)} 字符")
            logger.debug(f"RAG: 用户输入前100字符: \"{user_input[:100]}\"{'...' if len(user_input) > 100 else ''}")
            logger.debug(f"RAG: 当前索引大小: {self.chroma_collection.count() if self.chroma_collection else 0} 条目")
            logger.debug(f"RAG: 历史会话映射中: {len(self.historical_sessions_map)} 个会话文件")
        
        logger.start_loading_animation(
            message=f"{TermColors.MAGENTA}RAG系统正在翻阅历史记忆{TermColors.RESET}",
            animation_style='arrows', 
            color=TermColors.MAGENTA
        )
        
        rag_success_flag, rag_final_msg = False, "RAG检索完成"
        rag_context_messages = []
        
        total_start_time = datetime.now()
        
        try:
            rag_context_messages = self.get_relevant_messages(user_input)
            rag_success_flag = True
            if rag_context_messages:
                rag_final_msg = f"RAG检索完毕 (找到 {len(rag_context_messages)} 条相关历史)"
            else:
                rag_final_msg = "RAG检索完毕 (未找到相关历史)"
        except Exception as e_rag:
            logger.error(f"RAG检索过程中发生错误: {e_rag}")
            logger.debug(f"RAG retrieval error: {e_rag}", exc_info=True)
            rag_final_msg = "RAG检索失败"
            rag_success_flag = False
        finally:
            logger.stop_loading_animation(success=rag_success_flag, final_message=rag_final_msg)

        if not rag_context_messages:
            logger.debug("RAG: 未找到相关上下文，返回空列表")
            return []
            
        rag_prefix = getattr(self.config, 'RAG_PROMPT_PREFIX', '')
        rag_suffix = getattr(self.config, 'RAG_PROMPT_SUFFIX', '')
        
        system_prompts_in_results = []
        for msg in rag_context_messages:
            if msg.get('role') == 'system':
                content = msg.get('content', '')
                if content != rag_prefix and content != rag_suffix and not content.startswith('[历史对话片段'):
                    system_prompts_in_results.append(msg)
        
        if system_prompts_in_results:
            logger.warning_color("警告: RAG检索结果中包含系统提示词，这可能导致提示词重复", TermColors.YELLOW)
            if logger.should_print_context():
                for i, msg in enumerate(system_prompts_in_results):
                    content = msg.get('content', '')
                    shortened = content[:100] + ('...' if len(content) > 100 else '')
                    logger.debug(f"检测到的系统提示[{i+1}]: {shortened}")
            
            original_count = len(rag_context_messages)
            rag_context_messages = [msg for msg in rag_context_messages if msg.get('role') != 'system' or
                                   msg.get('content') == rag_prefix or
                                   msg.get('content') == rag_suffix or
                                   (msg.get('content', '').startswith('[历史对话片段'))]
            
            if logger.should_print_context():
                logger.debug(f"RAG: 从结果中过滤了 {original_count - len(rag_context_messages)} 条系统提示词")
            
        result_messages = []
        
        rag_prefix_content = getattr(self.config, 'RAG_PROMPT_PREFIX', 
            "以下是根据你的问题从历史对话中检索到的相关片段，其中包含了对话发生的大致时间：")
        if rag_prefix_content and rag_prefix_content.strip():
            result_messages.append({"role": "system", "content": rag_prefix_content})
            if logger.should_print_context():
                logger.debug(f"RAG: 添加前缀提示: \"{rag_prefix_content[:100]}\"{'...' if len(rag_prefix_content) > 100 else ''}")
            
        result_messages.extend(rag_context_messages)
        
        rag_suffix_content = getattr(self.config, 'RAG_PROMPT_SUFFIX', "")
        if rag_suffix_content and rag_suffix_content.strip():
            result_messages.append({"role": "system", "content": rag_suffix_content})
            if logger.should_print_context():
                logger.debug(f"RAG: 添加后缀提示: \"{rag_suffix_content[:100]}\"{'...' if len(rag_suffix_content) > 100 else ''}")
        
        if logger.should_print_context():
            role_counts = {}
            total_chars = 0
            for msg in result_messages:
                role = msg.get('role', 'unknown')
                role_counts[role] = role_counts.get(role, 0) + 1
                content = msg.get('content', '')
                total_chars += len(content)
                
            role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
            
            total_end_time = datetime.now()
            total_time_ms = (total_end_time - total_start_time).total_seconds() * 1000
            
            logger.debug(f"RAG: 最终准备了 {len(result_messages)} 条RAG消息，角色分布: {role_stats}")
            logger.debug(f"RAG: RAG内容总大小: {total_chars} 字符")
            logger.debug(f"RAG: 总准备时间: {total_time_ms:.2f}ms")
            logger.debug(f"------ RAG准备结束 ------\n")
            
        return result_messages