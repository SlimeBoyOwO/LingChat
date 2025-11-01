import sys
import os
import json
import uuid
import re
import requests  # 用于向嵌入服务发送HTTP请求
from datetime import datetime
from typing import List, Dict, Tuple, Optional, Any
from ling_chat.core.logger import logger, TermColors
from ling_chat.utils.runtime_path import third_party_path

_chromadb_imported_ok = True
try:
    import chromadb
    from chromadb.config import Settings
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
        # --- 修正点 ---
        # 明确检查 character_id 是否为 None，而不是使用会误判 0 的 `if not`
        if character_id is None:
            raise ValueError("RAGSystem必须使用一个有效的character_id进行初始化。")
    
        self.config = config
        self.character_id = character_id
        self.chroma_client = None
        self.chroma_collection = None
        self.historical_sessions_map = {}
        self.flat_historical_messages = []
    
        self.CHROMA_COLLECTION_NAME = f"rag_collection_char_{self.character_id}"
        # 从配置中读取嵌入服务的URL，并提供一个默认值
        self.EMBEDDING_SERVICE_URL = getattr(config, 'RAG_EMBEDDING_SERVICE_URL', 'http://localhost:5001/encode')

    def _get_embeddings(self, texts: List[str]) -> Optional[List[List[float]]]:
        """
        通过调用外部API服务来获取一批文本的嵌入向量。

        :param texts: 需要进行向量化处理的文本字符串列表。
        :return: 一个包含浮点数向量的列表，如果API调用失败则返回 None。
        """
        if not texts:
            return []
        
        try:
            payload = {"texts": texts}
            # 设置合理的超时时间，防止长时间等待
            response = requests.post(self.EMBEDDING_SERVICE_URL, json=payload, timeout=30)
            response.raise_for_status()  # 如果HTTP状态码不是2xx，则抛出异常
            data = response.json()
            return data.get("embeddings")
        except requests.exceptions.RequestException as e:
            logger.error(f"RAG: 调用嵌入服务失败: {e}")
            logger.debug(f"RAG: Embedding service request error: {e}", exc_info=True)
            return None

    def initialize(self) -> bool:
        '''
        初始化RAG系统的所有核心组件。连接到ChromaDB并检查嵌入服务的可用性。
    
        :return: bool: 如果所有组件都成功初始化，则返回 `True`，否则返回 `False`。
        '''
        if not getattr(self.config, 'USE_RAG', False):
            logger.info("RAG功能已禁用 (根据配置)。")
            return False
    
        if not _chromadb_imported_ok:
            logger.error(f"RAG组件初始化失败: 'chromadb' 模块未找到。请运行: pip install chromadb")
            return False
    
        logger.debug("开始初始化RAG组件...")
        try:
            # 步骤 1: 检查嵌入服务的健康状况
            health_url = self.EMBEDDING_SERVICE_URL.replace('/encode', '/health')
            logger.info(f"RAG: 正在检查嵌入服务的可用性 at {health_url}")
            try:
                response = requests.get(health_url, timeout=5)
                if response.status_code == 200 and response.json().get('status') == 'ok':
                    logger.info_color("RAG: 嵌入服务连接成功且状态正常。", TermColors.GREEN)
                else:
                    logger.error(f"RAG: 嵌入服务状态异常: {response.status_code} - {response.text}")
                    return False
            except requests.exceptions.RequestException as e:
                logger.error(f"RAG: 无法连接到嵌入服务: {e}")
                logger.error("请确保 'embedding_service.py' 脚本正在独立运行。")
                return False

            # 步骤 2: 初始化ChromaDB
            chroma_db_path = getattr(self.config, 'CHROMA_DB_PATH', './chroma_db_store')
            logger.debug(f"RAG: 初始化ChromaDB客户端 (记忆库将存储在 '{chroma_db_path}').")
            self.chroma_client = chromadb.PersistentClient(path=chroma_db_path, 
                                                           settings=Settings(anonymized_telemetry=False))
            logger.debug(f"RAG: ChromaDB客户端初始化成功 (数据路径: {chroma_db_path})。")
    
            logger.debug(f"RAG: 获取或创建ChromaDB集合: {self.CHROMA_COLLECTION_NAME}")
            self.chroma_collection = self.chroma_client.get_or_create_collection(
                name=self.CHROMA_COLLECTION_NAME,
                metadata={"hnsw:space": "cosine"}
            )
            logger.info(
                f"RAG: 角色 {self.character_id} 的ChromaDB集合 '{self.CHROMA_COLLECTION_NAME}' 已就绪。当前包含 {self.chroma_collection.count()} 条目。")
    
            self.load_historical_data()
    
            return True
        except Exception as e:
            logger.error(f"RAG组件初始化过程中发生错误: {e}")
            logger.debug(f"RAG Initialization Error during component setup: {e}", exc_info=True)
            self.chroma_client = None
            self.chroma_collection = None
            return False
    
    def load_historical_data(self) -> Tuple[int, int]:
        '''
        从指定目录加载历史对话JSON文件并建立索引。用于在系统启动时填充RAG系统的知识库，使其能够检索过去的对话。
    
        :return: Tuple[int, int]: 一个元组，包含成功加载的会话文件数量和消息总数。
        '''
        if not getattr(self.config, 'USE_RAG', False) or not self.chroma_collection:
            logger.debug("RAG: 组件未初始化或RAG已禁用，跳过历史数据加载。")
            return 0, 0
    
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
    
        self.flat_historical_messages = all_messages_flat
        self.historical_sessions_map = historical_sessions_map
    
        if all_messages_flat:
            self.add_messages_to_index(all_messages_flat)
    
        return loaded_files_count, total_messages_loaded
    
    def _parse_session_time_from_filename(self, filename: str) -> str:
        '''
        从会话历史文件名中解析出格式化的时间字符串。
        '''
        match = re.search(r"session_(\d{8}_\d{6})\.json", filename)
        if match:
            try:
                dt_obj = datetime.strptime(match.group(1), "%Y%m%d_%H%M%S")
                return dt_obj.strftime("%Y年%m月%d日 %H:%M")
            except ValueError:
                return "未知时间"
        return "未知时间"
    
    def add_messages_to_index(self, messages_with_metadata: List[Dict]) -> bool:
        '''
        将一批消息添加到向量数据库索引中。通过API获取向量嵌入，并将其与元数据一同存入ChromaDB。
        '''
        if not getattr(self.config, 'USE_RAG', False) or not self.chroma_collection:
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
    
        logger.debug(f"RAG: 正在为 {len(documents)} 个文档调用嵌入服务生成向量...")
        embeddings = self._get_embeddings(documents)
        
        if embeddings is None:
            logger.error("RAG: 从嵌入服务获取向量失败。索引操作已中止。")
            return False

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
            logger.debug(f"RAG: 成功向ChromaDB中添加/更新了 {len(ids)} 个文档。")
            return True
        except Exception as e:
            logger.error(f"RAG: 向ChromaDB中Upsert文档时出错: {e}")
            logger.debug(f"RAG: ChromaDB Upsert Error: {e}", exc_info=True)
            return False
    
    def add_session_to_history(self, session_messages: List[Dict], session_filepath: Optional[str] = None) -> Optional[str]:
        '''
        将一次完整的对话会话保存到历史记录文件，并更新RAG索引。
        '''
        if not getattr(self.config, 'USE_RAG', False):
            return None
        if not session_messages:
            return None
    
        filtered_messages = [msg for msg in session_messages if msg.get('role') != 'system']
        if not filtered_messages:
            return None
    
        try:
            if not session_filepath:
                session_filepath = self.get_history_filepath()
    
            os.makedirs(os.path.dirname(session_filepath), exist_ok=True)
    
            with open(session_filepath, 'w', encoding='utf-8') as f:
                json.dump(filtered_messages, f, ensure_ascii=False, indent=4)
            
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
            return session_filepath
        except Exception as e:
            logger.error(f"RAG: 保存会话历史失败: {e}")
            return None
    
    def get_history_filepath(self) -> str:
        '''
        为新的会话记录创建一个有组织的、基于日期的文件路径，按角色分目录存储。
        '''
        now = datetime.now()
        base_path = getattr(self.config, 'RAG_HISTORY_PATH', './rag_chat_history')
        character_specific_path = os.path.join(base_path, f"character_{self.character_id}")
        day_path = os.path.join(character_specific_path, now.strftime("%Y年%m月"), now.strftime("%d日"))
        os.makedirs(day_path, exist_ok=True)
        session_start_time_str = now.strftime("%Y%m%d_%H%M%S")
        return os.path.join(day_path, f"session_{session_start_time_str}.json")
    
    def get_relevant_messages(self, query_text: str) -> List[Dict]:
        '''
        核心检索功能。通过API获取查询向量，在ChromaDB中找到最相似的历史对话点，并提取上下文片段。
        '''
        if not getattr(self.config, 'USE_RAG', False) or not self.chroma_collection:
            return []
        if not query_text or self.chroma_collection.count() == 0:
            return []
    
        start_time = datetime.now()
        retrieval_count = getattr(self.config, 'RAG_RETRIEVAL_COUNT', 3)
        candidate_multiplier = getattr(self.config, 'RAG_CANDIDATE_MULTIPLIER', 3)
        context_before = getattr(self.config, 'RAG_CONTEXT_M_BEFORE', 2)
        context_after = getattr(self.config, 'RAG_CONTEXT_N_AFTER', 2)
        num_candidates_to_fetch = min(retrieval_count * candidate_multiplier, self.chroma_collection.count())
    
        logger.info_color(f"RAG: 正在为查询 \"{query_text[:50]}...\" 检索...", TermColors.BLUE)
    
        vector_start_time = datetime.now()
        query_embeddings = self._get_embeddings([query_text])
        if not query_embeddings:
            logger.error("RAG: 获取查询向量失败，无法进行检索。")
            return []
        query_embedding = query_embeddings[0]
        vector_time_ms = (datetime.now() - vector_start_time).total_seconds() * 1000
        logger.debug(f"RAG: 查询向量计算耗时 (通过API): {vector_time_ms:.2f}ms")
    
        try:
            db_start_time = datetime.now()
            results = self.chroma_collection.query(
                query_embeddings=[query_embedding],
                n_results=num_candidates_to_fetch,
                include=["metadatas", "distances"]
            )
            db_time_ms = (datetime.now() - db_start_time).total_seconds() * 1000
            logger.debug(f"RAG: ChromaDB查询耗时: {db_time_ms:.2f}ms")
        except Exception as e:
            logger.error(f"RAG 查询ChromaDB失败: {e}")
            return []
    
        final_rag_messages, used_source_indices, added_message_contents = [], set(), set()
        retrieved_blocks_count = 0
    
        if results and results.get('ids') and results['ids'][0]:
            for i in range(len(results['ids'][0])):
                if retrieved_blocks_count >= retrieval_count:
                    break
                
                metadata = results['metadatas'][0][i]
                distance = results['distances'][0][i]
                source_file, original_idx = metadata.get("source_file"), metadata.get("original_idx")

                if (source_file, original_idx) in used_source_indices:
                    continue

                current_session_messages = self.historical_sessions_map.get(source_file)
                if not current_session_messages or not (0 <= original_idx < len(current_session_messages)):
                    continue

                start_idx = max(0, original_idx - context_before)
                end_idx = min(len(current_session_messages), original_idx + context_after + 1)
                
                context_block = []
                for j in range(start_idx, end_idx):
                    msg_obj = current_session_messages[j]
                    msg_content = msg_obj.get("content")
                    if msg_content and msg_content not in added_message_contents:
                        context_block.append(msg_obj)
                        used_source_indices.add((source_file, j))
                        added_message_contents.add(msg_content)

                if context_block:
                    retrieved_blocks_count += 1
                    logger.info_color(f"\nRAG 系统检索到历史对话片段 (核心距离: {distance:.4f}):", TermColors.MAGENTA)
                    for msg in context_block:
                        session_time_str = msg.get("_session_timestamp_str", "未知时间")
                        contextualized_content = f"[历史对话片段 - {session_time_str}] {msg['content']}"
                        final_rag_messages.append({"role": msg['role'], "content": contextualized_content})
                        logger.info_color(f"  - ({session_time_str}) [{msg['role']}]: \"{msg['content'][:50]}...\"", TermColors.MAGENTA)

        total_time_ms = (datetime.now() - start_time).total_seconds() * 1000
        if final_rag_messages:
            logger.info_color(f"RAG: 为LLM准备了 {len(final_rag_messages)} 条消息，来自 {retrieved_blocks_count} 个上下文块。", TermColors.GREEN)
            logger.debug(f"RAG: 总检索时间: {total_time_ms:.2f}ms")
        else:
            logger.info_color("RAG 系统: 未在历史记录中找到相关消息。", TermColors.YELLOW)
            
        return final_rag_messages
    
    def prepare_rag_messages(self, user_input: str) -> List[Dict]:
        '''
        为用户输入准备完整的RAG上下文消息列表，包括前后缀提示。
        '''
        if not getattr(self.config, 'USE_RAG', False):
            return []
    
        logger.start_loading_animation(message=f"{TermColors.MAGENTA}RAG系统正在翻阅历史记忆{TermColors.RESET}")
        
        rag_context_messages = []
        try:
            rag_context_messages = self.get_relevant_messages(user_input)
            success = True
            final_msg = f"RAG检索完毕 (找到 {len(rag_context_messages)} 条相关历史)" if rag_context_messages else "RAG检索完毕 (未找到相关历史)"
        except Exception as e:
            logger.error(f"RAG检索过程中发生错误: {e}")
            success = False
            final_msg = "RAG检索失败"
        finally:
            logger.stop_loading_animation(success=success, final_message=final_msg)

        if not rag_context_messages:
            return []
    
        result_messages = []
        rag_prefix = getattr(self.config, 'RAG_PROMPT_PREFIX', "以下是根据你的问题从历史对话中检索到的相关片段：")
        if rag_prefix.strip():
            result_messages.append({"role": "system", "content": rag_prefix})
    
        result_messages.extend(rag_context_messages)
    
        rag_suffix = getattr(self.config, 'RAG_PROMPT_SUFFIX', "")
        if rag_suffix.strip():
            result_messages.append({"role": "system", "content": rag_suffix})
    
        return result_messages