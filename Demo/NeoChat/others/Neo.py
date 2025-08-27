import sys
import config
from core.logger import (
    initialize_logger,
    start_loading_animation,
    stop_loading_animation,
    TermColors,
    log_debug,
    log_info,
    log_warning,
    log_error,
    log_info_color,
    log_warning_color,
    log_error_color
)
# 早期日志初始化，用于在python库导入期间显示动画
_early_init_app_name = getattr(config, 'AI_NAME', "App") + "_PreLoad"
initialize_logger(
    config_debug_mode=getattr(config, 'DEBUG_MODE', False),
    app_name=_early_init_app_name,
    show_timestamp=False  # 早期加载信息可以简洁些
)
log_debug("您目前处于开发者模式中，终端将会显示大量的灰色DEBUG日志，若要获得更好的使用体验，关闭开发者模式")
log_debug("正在加载Python依赖库，此过程可能较慢。")
start_loading_animation(
    message=f"{TermColors.CYAN}{config.AI_NAME}正在试图起床{TermColors.RESET}",
    animation_style_key='dots'
)

# 开始导入可能耗时的模块
import requests
import json
import os
from datetime import datetime, timezone
import uuid
import torch
import re

_sentence_transformer_imported_ok = True
_chromadb_imported_ok = True

try:
    from sentence_transformers import SentenceTransformer
except ImportError:
    _sentence_transformer_imported_ok = False
    if hasattr(config, 'USE_RAG') and config.USE_RAG:
        sys.stderr.write(
            f"{TermColors.RED}错误: 'sentence-transformers' 模块未找到，但 RAG 功能已启用。\n"
            f"请安装: pip install sentence-transformers{TermColors.RESET}\n")
        sys.stderr.flush()


    class SentenceTransformer:
        def __init__(self, *args, **kwargs): pass

        def encode(self, *args, **kwargs): raise NotImplementedError("SentenceTransformer is not available.")

try:
    import chromadb
except ImportError:
    _chromadb_imported_ok = False
    if hasattr(config, 'USE_RAG') and config.USE_RAG:
        sys.stderr.write(
            f"{TermColors.RED}错误: 'chromadb' 模块未找到，但 RAG 功能已启用。\n"
            f"请安装: pip install chromadb{TermColors.RESET}\n")
        sys.stderr.flush()

    class chromadb:
        class PersistentClient:
            def __init__(self, *args, **kwargs): pass

            def get_or_create_collection(self, *args, **kwargs): raise NotImplementedError(
                "chromadb is not available.")

        def get_collection(self, *args, **kwargs): raise NotImplementedError(
            "chromadb is not available.")

_early_load_successful = True
_early_load_message = "核心模块加载完成。"

if hasattr(config, 'USE_RAG') and config.USE_RAG:
    if not _sentence_transformer_imported_ok or not _chromadb_imported_ok:
        _early_load_successful = False
        missing_modules = []
        if not _sentence_transformer_imported_ok: missing_modules.append("'sentence-transformers'")
        if not _chromadb_imported_ok: missing_modules.append("'chromadb'")
        _early_load_message = f"核心RAG模块 ({', '.join(missing_modules)}) 加载失败。RAG功能可能受限。"

stop_loading_animation(success=_early_load_successful)

# --- 历史记录管理 ---
def get_history_filepath():
    now = datetime.now()
    year_month_path = os.path.join(config.HISTORY_BASE_PATH, now.strftime("%Y年%m月"))
    day_path = os.path.join(year_month_path, now.strftime("%d日"))
    os.makedirs(day_path, exist_ok=True)
    session_start_time_str = now.strftime("%Y%m%d_%H%M%S")
    return os.path.join(day_path, f"session_{session_start_time_str}.json")

def save_session_history(session_messages, filepath):
    try:
        with open(filepath, 'w', encoding='utf-8') as f:
            json.dump(session_messages, f, ensure_ascii=False, indent=4)
        log_debug(f"会话历史已保存到: {filepath}")
    except IOError as e:
        log_error_color(f"保存历史记录失败: {e}")
        log_debug(f"IOError saving history: {e}", exc_info=True)

def parse_session_time_from_filename(filename):
    match = re.search(r"session_(\d{8}_\d{6})\.json", filename)
    if match:
        try:
            dt_obj = datetime.strptime(match.group(1), "%Y%m%d_%H%M%S")
            return dt_obj.strftime("%Y年%m月%d日 %H:%M")
        except ValueError:
            log_debug(f"无法从文件名 {filename} 解析有效日期时间。")
            return "未知时间"
    return "未知时间"

def load_all_historical_data():
    all_messages_flat = []
    historical_sessions_map = {}

    if not os.path.exists(config.HISTORY_BASE_PATH):
        log_warning(f"历史记录基础路径 {config.HISTORY_BASE_PATH} 不存在。未加载任何历史。")
        return all_messages_flat, historical_sessions_map

    log_debug(f"开始从 {config.HISTORY_BASE_PATH} 加载历史对话数据...")
    loaded_files_count = 0
    total_messages_loaded = 0
    for root, _, files in os.walk(config.HISTORY_BASE_PATH):
        sorted_files = sorted([f for f in files if f.endswith(".json")])
        for filename in sorted_files:
            filepath = os.path.join(root, filename)
            session_time_str = parse_session_time_from_filename(filename)
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
                log_warning_color(f"加载历史文件 {filepath} 失败: {e}")
                log_debug(f"Failed to load history file {filepath}: {e}", exc_info=True)

    if loaded_files_count > 0:
        log_debug(f"成功从 {loaded_files_count} 个文件中加载了 {total_messages_loaded} 条历史消息。")
    else:
        log_warning_color("未找到或加载任何有效的历史会话文件。请检查Dialogue_history/文件是否正确存放，若您是初次使用本项目，请忽略此警报")
    log_debug(f"共映射 {len(historical_sessions_map)} 个会话。")
    return all_messages_flat, historical_sessions_map

# --- RAG 相关 ---
embedding_model = None
chroma_client = None
chroma_collection = None
CHROMA_COLLECTION_NAME = "chat_history_collection_v4"
EMBEDDING_MODEL_NAME = 'all-MiniLM-L6-v2'

def initialize_rag_components():
    global embedding_model, chroma_client, chroma_collection, _sentence_transformer_imported_ok, _chromadb_imported_ok
    if not config.USE_RAG:
        log_info("RAG功能已禁用 (根据配置)。")
        return False

    if not _sentence_transformer_imported_ok:
        log_error_color("RAG组件初始化失败: SentenceTransformer 模块未能成功导入。")
        return False
    if not _chromadb_imported_ok:
        log_error_color("RAG组件初始化失败: ChromaDB 模块未能成功导入。")
        return False

    log_debug("开始初始化RAG组件...")
    try:
        log_debug(f"RAG: 初始化Sentence Transformer模型: {EMBEDDING_MODEL_NAME}")
        device = 'cuda' if torch.cuda.is_available() else 'cpu'
        embedding_model = SentenceTransformer(EMBEDDING_MODEL_NAME, device=device)
        log_debug(f"RAG: Sentence Transformer模型 ({EMBEDDING_MODEL_NAME}) 加载成功 。当前使用 {device})进行RAG向量库匹配的推理。")

        chroma_db_path = getattr(config, 'CHROMA_DB_PATH', './chroma_db_store_v2')
        log_debug(f"RAG: 初始化ChromaDB客户端 (记忆库将存储在 '{chroma_db_path}').")
        chroma_client = chromadb.PersistentClient(path=chroma_db_path)
        log_debug(f"RAG: ChromaDB客户端初始化成功 (数据路径: {chroma_db_path})。")

        log_debug(f"RAG: 获取或创建ChromaDB集合: {CHROMA_COLLECTION_NAME}")
        chroma_collection = chroma_client.get_or_create_collection(
            name=CHROMA_COLLECTION_NAME,
            metadata={"hnsw:space": "cosine"}
        )
        log_debug(
            f"RAG: ChromaDB集合 '{CHROMA_COLLECTION_NAME}' 已就绪。当前包含 {chroma_collection.count()} 条目。")
        return True
    except Exception as e:
        log_error_color(f"RAG组件初始化过程中发生错误: {e}")
        log_debug(f"RAG Initialization Error during component setup: {e}", exc_info=True)
        embedding_model = None
        chroma_client = None
        chroma_collection = None
        return False


def add_messages_to_rag_index(messages_with_metadata):
    global embedding_model, chroma_collection
    if not config.USE_RAG or not embedding_model or not chroma_collection:
        log_debug("RAG: 组件未初始化或RAG已禁用，跳过索引。")
        return

    if not messages_with_metadata:
        log_info("RAG: 无消息可供索引。")
        return

    log_debug(f"RAG: 准备为 {len(messages_with_metadata)} 条消息建立索引...")
    documents, metadatas, ids = [], [], []

    for msg in messages_with_metadata:
        content, role = msg.get('content'), msg.get('role')
        source_file, original_idx = msg.get('_source_file'), msg.get('_original_idx')

        if not all([content, isinstance(content, str), role, source_file is not None, original_idx is not None]):
            log_debug(f"RAG: 跳过无效消息进行索引 (字段缺失): {str(msg)[:100]}...")
            continue

        message_id_str = f"{source_file}_{original_idx}_{role}_{content[:100]}"
        message_id = str(uuid.uuid5(uuid.NAMESPACE_DNS, message_id_str))
        documents.append(content)
        metadatas.append({"role": role, "source_file": source_file, "original_idx": original_idx})
        ids.append(message_id)

    if not documents:
        log_warning_color("RAG: 筛选后无有效文档可供索引。")
        return

    log_debug(f"RAG: 正在为 {len(documents)} 个文档生成嵌入向量...")
    embeddings = embedding_model.encode(documents).tolist()
    log_debug(f"RAG: 嵌入向量生成完毕。Shape: ({len(embeddings)}, {len(embeddings[0]) if embeddings else 0})")

    try:
        batch_size = 500
        for i in range(0, len(ids), batch_size):
            batch_ids, batch_embeddings = ids[i:i + batch_size], embeddings[i:i + batch_size]
            batch_documents, batch_metadatas = documents[i:i + batch_size], metadatas[i:i + batch_size]
            chroma_collection.upsert(ids=batch_ids, embeddings=batch_embeddings, documents=batch_documents,
                                     metadatas=batch_metadatas)
            log_debug(f"RAG: Upserted batch {i // batch_size + 1} ({len(batch_ids)} documents).")
        log_debug(f"RAG: 成功向ChromaDB中添加/更新了 {len(ids)} 个文档。")
        log_debug(f"RAG: 索引库 '{CHROMA_COLLECTION_NAME}' 当前总条目: {chroma_collection.count()}")
    except Exception as e:
        log_error_color(f"RAG: 向ChromaDB中Upsert文档时出错: {e}")
        log_debug(f"ChromaDB Upsert Error: {e}", exc_info=True)


def get_rag_messages_chroma(query_text, historical_sessions_map):
    global embedding_model, chroma_collection
    if not config.USE_RAG or not embedding_model or not chroma_collection:
        log_warning_color("RAG: 组件未初始化或RAG已禁用，跳过检索。")
        return []
    if not query_text:
        log_warning_color("RAG: 查询文本为空，跳过RAG检索。")
        return []
    if chroma_collection.count() == 0:
        log_warning_color("RAG: ChromaDB集合为空，跳过RAG检索。")
        return []

    num_candidates_to_fetch = config.RAG_RETRIEVAL_COUNT * config.RAG_CANDIDATE_MULTIPLIER
    num_candidates_to_fetch = min(num_candidates_to_fetch, chroma_collection.count())

    log_info_color(f"RAG: 正在为查询 \"{query_text[:50]}...\" 检索最多 {num_candidates_to_fetch} 个候选片段...",
                   TermColors.BLUE)
    query_embedding = embedding_model.encode([query_text], show_progress_bar=False)[0].tolist()

    try:
        results = chroma_collection.query(
            query_embeddings=[query_embedding],
            n_results=num_candidates_to_fetch,
            include=["documents", "metadatas", "distances"]
        )
    except Exception as e:
        log_error_color(f"RAG 查询ChromaDB失败: {e}")
        log_debug(f"ChromaDB Query Error: {e}", exc_info=True)
        return []

    final_rag_messages, used_chroma_doc_ids, added_message_contents_to_llm = [], set(), set()
    retrieved_blocks_count = 0

    if results and results.get('ids') and results['ids'][0]:
        log_debug(f"RAG: ChromaDB返回 {len(results['ids'][0])} 个候选结果。")
        for i in range(len(results['ids'][0])):
            if retrieved_blocks_count >= config.RAG_RETRIEVAL_COUNT:
                log_debug(f"RAG: 已达到期望的 {config.RAG_RETRIEVAL_COUNT} 个独立上下文块。停止处理候选。")
                break
            try:
                core_doc_id, core_doc_content = results['ids'][0][i], results['documents'][0][i]
                metadata, distance = results['metadatas'][0][i], results['distances'][0][i]
            except (IndexError, TypeError, KeyError) as e:
                log_warning(f"RAG: ChromaDB结果索引 {i} 处数据不完整或格式错误。跳过。详细: {e}")
                continue

            if core_doc_id in used_chroma_doc_ids or core_doc_content == query_text:
                log_debug(f"RAG: 跳过已使用或与查询相同的文档 ID {core_doc_id}.")
                continue

            source_file, original_idx = metadata.get("source_file"), metadata.get("original_idx")
            if source_file not in historical_sessions_map or not isinstance(original_idx, int):
                log_warning(f"RAG: 文档 {core_doc_id} 元数据不完整或会话未在Map中找到。跳过。")
                continue

            current_session_messages = historical_sessions_map[source_file]
            if not (0 <= original_idx < len(current_session_messages)):
                log_warning(f"RAG: 原始索引 {original_idx} 超出 '{source_file}' 会话边界。跳过。")
                continue

            start_idx = max(0, original_idx - config.RAG_CONTEXT_M_BEFORE)
            end_idx = min(len(current_session_messages), original_idx + config.RAG_CONTEXT_N_AFTER + 1)

            context_block_for_llm, context_block_display_info, potential_block_messages = [], [], []
            for j in range(start_idx, end_idx):
                msg_obj = current_session_messages[j]
                msg_content, msg_role = msg_obj.get("content"), msg_obj.get("role", "unknown")
                session_time_str = msg_obj.get("_session_timestamp_str", "未知时间")

                if msg_content and msg_content not in added_message_contents_to_llm:
                    contextualized_content = f"[历史对话片段 - {session_time_str}] {msg_content}"
                    potential_block_messages.append({"role": msg_role, "content": contextualized_content})
                    is_core = " (核心检索)" if j == original_idx else ""
                    context_block_display_info.append(
                        f"  - ({session_time_str}) [{msg_role}]{is_core}: \"{msg_content[:50]}...\"")

            if potential_block_messages:
                context_block_for_llm.extend(potential_block_messages)
                for msg in potential_block_messages: added_message_contents_to_llm.add(msg['content'])
                used_chroma_doc_ids.add(core_doc_id)
                retrieved_blocks_count += 1
                final_rag_messages.extend(context_block_for_llm)
                log_info_color(
                    f"\nRAG 系统检索到历史对话片段 (核心距离: {distance:.4f}, 源: {source_file}, 核心索引: {original_idx}):",
                    # MODIFIED
                    TermColors.MAGENTA)
                for line in context_block_display_info:
                    log_info_color(f"\n{line}", TermColors.MAGENTA)  # MODIFIED
                log_debug(f"RAG: 添加上下文块 (ID {core_doc_id}). LLM的RAG消息总数: {len(final_rag_messages)}")
            else:
                log_debug(f"RAG: 核心文档ID {core_doc_id} 的上下文块为空或所有消息已去重。")

    if not final_rag_messages:
        log_info_color("RAG 系统: 未在历史记录中找到与当前问题相关的、非重复的消息。", TermColors.YELLOW)
    else:
        log_info_color(
            f"RAG: 为LLM准备了 {len(final_rag_messages)} 条消息，来自 {retrieved_blocks_count} 个不同的RAG上下文块。",
            TermColors.GREEN)
    return final_rag_messages

# --- DeepSeek API 调用 ---
def generate_chat_response(messages_payload):
    headers = {"Content-Type": "application/json", "Authorization": f"Bearer {config.API_KEY}"}
    payload = {
        "model": config.MODEL_NAME, "messages": messages_payload, "stream": True,
        "max_tokens": config.MAX_TOKENS, "temperature": config.TEMPERATURE
    }

    if config.DEBUG_MODE:
        log_debug("--- 发送给 DeepSeek API 的 Payload (内容已截断) ---")
        debug_payload_display = json.loads(json.dumps(payload))
        for msg in debug_payload_display.get("messages", []):
            if 'content' in msg and isinstance(msg['content'], str):
                msg['content'] = msg['content'][:150] + ("..." if len(msg['content']) > 150 else "")
        formatted_payload_str = json.dumps(debug_payload_display, ensure_ascii=False, indent=2)
        for line in formatted_payload_str.splitlines(): log_debug(line)
        log_debug("--- Payload 结束 ---")

    assistant_full_response = ""
    api_call_succeeded = False
    animation_stopped_internally = False

    try:
        log_info_color(f"{config.AI_NAME}正在连接DeepSeek ({config.MODEL_NAME})... 请稍候。", TermColors.BLUE)
        start_loading_animation(
            message=f"{TermColors.LIGHT_BLUE}{config.AI_NAME}正在发呆{TermColors.RESET}",
            animation_style_key='moon',
            animation_color=TermColors.LIGHT_BLUE
        )

        response = requests.post(config.API_URL, headers=headers, json=payload, stream=True,
                                 timeout=config.API_TIMEOUT_SECONDS)
        response.raise_for_status()

        first_chunk_received = False
        for chunk in response.iter_lines():
            if chunk:
                decoded_line = chunk.decode('utf-8')
                if decoded_line.startswith("data: "):
                    json_data_str = decoded_line[len("data: "):]
                    if json_data_str.strip() == "[DONE]":
                        log_debug("API Stream: [DONE] 标记已收到。")
                        break
                    try:
                        data = json.loads(json_data_str)
                        content_piece = data.get("choices", [{}])[0].get("delta", {}).get("content", "")
                        if content_piece:
                            if not first_chunk_received:
                                stop_loading_animation(success=True)
                                animation_stopped_internally = True
                                print(f"{TermColors.CYAN}{config.AI_NAME}: {TermColors.RESET}", end="", flush=True)
                                first_chunk_received = True
                            sys.stdout.write(f"{TermColors.CYAN}{content_piece}{TermColors.RESET}")
                            sys.stdout.flush()
                            assistant_full_response += content_piece
                    except (json.JSONDecodeError, IndexError):
                        log_warning(f"API Stream: 解码或索引错误，数据块: {json_data_str}")

        if first_chunk_received:
            print();
            api_call_succeeded = True
        elif response.ok:
            log_info("API 响应流结束，但未返回任何文本内容。");
            api_call_succeeded = True

    except requests.exceptions.HTTPError as e_http:
        log_error_color(f"\nAPI请求HTTP错误: {e_http} - {e_http.response.status_code} {e_http.response.reason}")
        try:
            log_error_color(f"错误详情: {json.dumps(e_http.response.json(), ensure_ascii=False, indent=2)}")
        except ValueError:
            log_error_color(f"错误响应体 (非JSON): {e_http.response.text}")
        log_debug(f"API HTTPError: {e_http}", exc_info=True)
    except requests.exceptions.Timeout:
        log_error_color(f"\nAPI请求超时 (超过 {config.API_TIMEOUT_SECONDS} 秒)。")
        log_debug("API Request Timeout", exc_info=True)
    except requests.exceptions.RequestException as e_req:
        log_error_color(f"\nAPI请求失败: {e_req}")
        log_debug(f"API Request Exception: {e_req}", exc_info=True)
    except Exception as e_unknown:
        log_error_color(f"\n处理API响应时发生未知错误: {e_unknown}")
        log_debug(f"Unknown error during API response processing: {e_unknown}", exc_info=True)
    finally:
        if not animation_stopped_internally:
            final_msg = None
            if not api_call_succeeded:
                final_msg = "与API的通信出现问题"
            elif not assistant_full_response and api_call_succeeded:
                final_msg = "API已响应 (无文本内容)"
            stop_loading_animation(success=api_call_succeeded, final_message=final_msg)

    if api_call_succeeded and assistant_full_response:
        log_debug(f"API完整响应已接收 (长度: {len(assistant_full_response)}).")
        return assistant_full_response
    return None


# --- 主程序 ---
def main():
    initialize_logger(config_debug_mode=config.DEBUG_MODE, app_name=f"{config.AI_NAME}_ChatRAG")

    rag_initialized_successfully = False
    flat_historical_messages, historical_sessions_map = [], {}

    start_loading_animation(
        message=f"{TermColors.CYAN}{config.AI_NAME}正在整理回忆思绪{TermColors.RESET}",
        animation_style_key='dots')

    init_success = False
    init_final_message = "系统初始化失败" # 默认失败消息

    try:
        if initialize_rag_components():
            rag_initialized_successfully = True
            log_debug("开始加载历史记录并更新RAG索引...")
            flat_historical_messages, historical_sessions_map = load_all_historical_data()

            if flat_historical_messages and chroma_collection is not None:
                add_messages_to_rag_index(flat_historical_messages)
                init_final_message = f"程序已就绪"
                log_debug("系统初始化完成。RAG索引包含 {chroma_collection.count()} 条记录。")
            elif chroma_collection is not None:
                init_final_message = "系统初始化完成。RAG就绪 (无历史数据索引)。"
            else:
                init_final_message = "系统初始化完成，但RAG数据处理异常或RAG未启用。"
            init_success = True # RAG组件初始化成功并处理完数据，标记为成功

        else:
            log_info_color("RAG组件初始化失败。尝试仅加载历史记录...", TermColors.YELLOW)
            flat_historical_messages, historical_sessions_map = load_all_historical_data()
            init_final_message = "RAG组件初始化失败。RAG功能将不可用。历史记录已加载（如果存在）。"
            init_success = True # 加载历史记录本身可以认为是部分的成功

    except Exception as e:
        log_error_color(f"初始化过程中发生意外严重错误: {e}")
        log_debug(f"Unexpected initialization error: {e}", exc_info=True)
        init_final_message = "初始化过程中发生严重错误"
        init_success = False # 任何非预期的错误都标记为失败
    finally:
        stop_loading_animation(success=init_success, final_message=init_final_message)

    if rag_initialized_successfully and chroma_collection:
        log_debug(f"RAG已准备就绪，知识库包含 {chroma_collection.count()} 条向量化历史消息。")

    elif config.USE_RAG:
        log_debug("RAG初始化失败或历史为空。问答可能仅依赖当前会话。")

    else:
        log_info("RAG功能已禁用。问答将仅依赖当前会话。")


    # --- 会话主循环开始 ---
    current_session_messages = []
    session_filepath = get_history_filepath()
    print(f"{TermColors.GREY}输入 'exit' 或 'quit' 退出。本次会话记录到: {session_filepath}{TermColors.RESET}")

    while True:
        try:
            user_input = input(f"{TermColors.YELLOW}你: {TermColors.RESET}")
        except UnicodeDecodeError:
            log_error_color("系统检测到无法识别的输入字符。");
            continue
        except EOFError:
            print("\n再见！(EOF)");
            break
        except KeyboardInterrupt:
            print("\n再见！(中断)");
            break

        if user_input.lower() in ["exit", "quit", "退出"]:
            print("再见！");
            break
        if not user_input.strip():
            continue

        api_payload_messages = []

        # --- 步骤 A: 准备各种消息组件 ---

        # A1. 获取当前时间并创建时间提示 (每次请求都获取)
        current_time_str = datetime.now().strftime("%Y年%m月%d日 %H:%M:%S")
        time_system_message = {"role": "system", "content": f"assistant_hint: 当前提问时间是 {current_time_str}。"}
        # log_debug 移动到添加时记录

        # A2. 准备配置中的系统提示 (如果存在)
        config_system_prompt_message = None
        if config.SYSTEM_PROMPT and config.SYSTEM_PROMPT.strip():
            config_system_prompt_message = {"role": "system", "content": config.SYSTEM_PROMPT}
            # log_debug 移动到添加时记录

        # A3. 处理 RAG 检索，并将结果收集到 rag_messages_to_add 列表
        rag_messages_to_add = []
        if config.USE_RAG and rag_initialized_successfully and chroma_collection and chroma_collection.count() > 0:
            start_loading_animation(
                message=f"{TermColors.MAGENTA}{config.AI_NAME}正在翻看记事本{TermColors.RESET}",
                animation_style_key='arrows', animation_color=TermColors.MAGENTA
            )
            rag_success_flag, rag_final_msg = False, "RAG检索完成"
            rag_context_messages = []
            try:
                rag_context_messages = get_rag_messages_chroma(user_input, historical_sessions_map)
                rag_success_flag = True
                if rag_context_messages:
                    rag_final_msg = f"RAG检索完毕 (找到 {len(rag_context_messages)} 条相关历史)"
                else:
                    rag_final_msg = "RAG检索完毕 (未找到相关历史)"
            except Exception as e_rag:
                log_error_color(f"RAG检索过程中发生错误: {e_rag}")
                log_debug(f"RAG retrieval error: {e_rag}", exc_info=True)
                rag_final_msg = "RAG检索失败"
                rag_success_flag = False # RAG 检索失败
            finally:
                stop_loading_animation(success=rag_success_flag, final_message=rag_final_msg)

            if rag_context_messages:
                rag_prefix_content = config.RAG_PROMPT_PREFIX
                if not rag_prefix_content or not rag_prefix_content.strip():
                    rag_prefix_content = "以下是根据你的问题从历史对话中检索到的相关片段，其中包含了对话发生的大致时间："
                rag_messages_to_add.append({"role": "system", "content": rag_prefix_content})
                rag_messages_to_add.extend(rag_context_messages)
                if config.RAG_PROMPT_SUFFIX and config.RAG_PROMPT_SUFFIX.strip():
                    rag_messages_to_add.append({"role": "system", "content": config.RAG_PROMPT_SUFFIX})
                # log_debug 移动到添加时记录
        else:
            reasons = [r for r, c in [("USE_RAG为False", not config.USE_RAG),
                                      ("RAG未成功初始化", not rag_initialized_successfully),
                                      ("Chroma集合不可用", chroma_collection is None),
                                      ("Chroma集合为空",
                                       chroma_collection is not None and chroma_collection.count() == 0)] if c]
            if reasons: log_debug(f"跳过RAG检索，原因: {', '.join(reasons)}。")

        # --- 步骤 B: 按新顺序组装 api_payload_messages ---

        # B1. 添加 RAG 消息 (如果存在)
        if rag_messages_to_add:
            api_payload_messages.extend(rag_messages_to_add)
            log_debug(f"已添加 {len(rag_messages_to_add)} 条RAG消息(包括前后缀)。")

        # B2. 添加配置中的系统提示 (在RAG之后)
        if config_system_prompt_message:
            api_payload_messages.append(config_system_prompt_message)
            log_debug(f"已添加系统提示: \"{config_system_prompt_message['content'][:100].strip().replace(chr(10), ' ')}...\"")

        # B3. 添加当前时间提示 (在配置系统提示之后)
        api_payload_messages.append(time_system_message)
        log_debug(f"已添加当前时间提示: \"{time_system_message['content']}\"")

        # B4. 添加当前会话的滑动窗口历史
        temp_sliding_window = current_session_messages[-(
            config.MAX_CONTEXT_MESSAGES_SLIDING_WINDOW - 1 if config.MAX_CONTEXT_MESSAGES_SLIDING_WINDOW > 0 else 0):]
        if temp_sliding_window: # 仅当有历史时才添加和记录
            api_payload_messages.extend(temp_sliding_window)
            log_debug(f"已从当前会话添加 {len(temp_sliding_window)} 条历史消息。")
        else:
            log_debug("当前会话无历史消息可添加。")


        # B5. 添加当前用户输入消息
        user_message_for_payload = {"role": "user", "content": user_input}
        api_payload_messages.append(user_message_for_payload)
        log_debug(f"已添加当前用户消息。最终Payload消息总数: {len(api_payload_messages)}")

        current_session_messages.append(user_message_for_payload)

        assistant_response = generate_chat_response(api_payload_messages)

        if assistant_response and assistant_response.strip():
            current_session_messages.append({"role": "assistant", "content": assistant_response})
            save_session_history(current_session_messages, session_filepath)
        else:
            log_warning_color("API调用未返回有效响应或响应为空。", TermColors.YELLOW)
            if current_session_messages and current_session_messages[-1]["role"] == "user":
                log_debug("由于API调用失败/响应无效，从当前会话记录中移除最后用户消息。")
                current_session_messages.pop()

if __name__ == "__main__":
    if not hasattr(config, 'API_KEY') or not config.API_KEY or \
            config.API_KEY.lower() in ["your_deepseek_api_key", "sk-114514", "sk-1234"] or \
            "actual_deepseek_api_key" in config.API_KEY.lower():
        log_error("错误：请在 config.py 文件中正确设置您的 DeepSeek API_KEY。")

    default_history_path = "./chat_history_data"
    if not hasattr(config, 'HISTORY_BASE_PATH') or not config.HISTORY_BASE_PATH:
        sys.stderr.write(
            f"{TermColors.YELLOW}警告: config.py 中未定义或 HISTORY_BASE_PATH 为空。将使用默认路径: {default_history_path}{TermColors.RESET}\n")
        sys.stderr.flush()
        setattr(config, 'HISTORY_BASE_PATH', default_history_path)
    try:
        os.makedirs(config.HISTORY_BASE_PATH, exist_ok=True)
    except OSError as e:
        sys.stderr.write(
            f"{TermColors.RED}错误: 无法创建历史记录目录 {config.HISTORY_BASE_PATH}: {e}{TermColors.RESET}\n")
        sys.stderr.flush()

    main()