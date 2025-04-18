import os
import json
import asyncio
import websockets
import glob
from deepseek import DeepSeek
import re
from predictor import EmotionClassifier  # 导入情绪分类器
from VitsTTS import VitsTTS              # 导入语音生成
from logger import Logger
from langDetect import LangDetect
import dotenv
# 这部分是新增的
import subprocess # 用于运行外部命令 (Node.js)
import webbrowser # 用于打开浏览器
import time       # 用于添加延迟
import atexit     # 用于注册退出时执行的函数
import sys        # 用于获取可执行文件路径等


dotenv.load_dotenv()
logger = Logger()
deepseek = DeepSeek()
emotion_classifier = EmotionClassifier()
langDetect = LangDetect()
tts_engine = VitsTTS(
    #enbale=False      #如果你没有配置simple-voice-api，请去掉这一行最开始的#号
)

temp_voice_dir = os.environ.get("TEMP_VOICE_DIR", "frontend/public/audio")
os.makedirs(temp_voice_dir, exist_ok=True)

# ANSI 颜色代码
COLOR_USER = "\033[92m"  # 绿色
COLOR_AI = "\033[96m"    # 青蓝色
COLOR_EMOTION = "\033[93m" # 黄色（用于情绪显示）
COLOR_RESET = "\033[0m"  # 重置颜色

# --- 新增全局变量，用于持有 Node.js 进程 ---
node_process = None
# --- 结束新增全局变量 ---

# --- 新增函数：终止 Node.js 进程 ---
def kill_node_process():
    """尝试终止正在运行的 Node.js 前端服务器进程"""
    global node_process
    if node_process and node_process.poll() is None: # 检查进程是否存在且仍在运行
        print("\n正在尝试终止 Node.js 前端服务器...")
        try:
            node_process.terminate() # 发送终止信号
            node_process.wait(timeout=5) # 等待最多5秒让其终止
            print("Node.js 前端服务器已终止。")
        except subprocess.TimeoutExpired:
            print("警告：Node.js 进程未能及时终止，尝试强制终止...")
            node_process.kill() # 强制终止
            node_process.wait()
            print("Node.js 前端服务器已被强制终止。")
        except Exception as e:
            print(f"终止 Node.js 进程时出错: {e}")
    node_process = None # 重置变量
# --- 结束新增函数 ---

def analyze_emotions(text):
    """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
    # 改进后的正则表达式，更灵活地匹配各种情况
    emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)

    results = []
    for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):
        # 统一处理括号（兼容中英文括号）
        following_text = following_text.replace('(', '（').replace(')', '）')

        # 提取日语部分（<...>），改进匹配模式
        japanese_match = re.search(r'<(.*?)>', following_text)
        japanese_text = japanese_match.group(1).strip() if japanese_match else ""

        # 提取动作部分（（...）），改进匹配模式
        motion_match = re.search(r'（(.*?)）', following_text)
        motion_text = motion_match.group(1).strip() if motion_match else ""

        # 清理后的文本（移除日语部分和动作部分）
        cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()

        # 清理日语文本中的动作部分
        if japanese_text:
            japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

        # 跳过完全空的文本
        # 修改：如果cleaned_text和japanese_text都为空，但motion_text不为空，也应保留
        if not cleaned_text and not japanese_text and not motion_text:
             continue # 只有三者都为空时才跳过

        # 改进语言检测和处理
        try:
            if japanese_text and cleaned_text:
                # 如果两者都有内容，才进行语言检测和交换
                lang_jp = langDetect.detect_language(japanese_text)
                lang_clean = langDetect.detect_language(cleaned_text)

                if (lang_jp in ['Chinese', 'Chinese_ABS'] and lang_clean in ['Japanese', 'Chinese']) and \
                    lang_clean != 'Chinese_ABS':
                        cleaned_text, japanese_text = japanese_text, cleaned_text


        except Exception as e:
            # 语言检测失败时保持原样
            print(f"语言检测错误: {e}")

        # 对情绪标签单独预测，增加错误处理
        try:
            predicted = emotion_classifier.predict(emotion_tag)
            prediction_result = {
                "label": predicted["label"],
                "confidence": predicted["confidence"]
            }
        except Exception as e:
            print(f"情绪预测错误 '{emotion_tag}': {e}")
            prediction_result = {
                "label": "unknown",
                "confidence": 0.0
            }

        results.append({
            "index": i,
            "original_tag": emotion_tag,
            "following_text": cleaned_text,
            "motion_text": motion_text,
            "japanese_text": japanese_text,
            "predicted": prediction_result["label"],
            "confidence": prediction_result["confidence"],
            # 使用 os.path.basename 确保只包含文件名
            "voice_file": os.path.join(temp_voice_dir, f"part_{i}.{tts_engine.format}")
        })

    return results

async def generate_voice_files(text_segments):
    """异步生成所有语音文件"""
    tasks = []
    for segment in text_segments:
        # 修改：即使没有日语，如果有中文文本，也可能需要语音（取决于TTS引擎配置）
        # 这里我们假设TTS主要用于日语，如果需要中文TTS，逻辑需要调整
        text_to_speak = segment["japanese_text"]
        if text_to_speak:  # 确保有实际内容需要合成
            task = text_to_speech(
                text_to_speak,
                segment["voice_file"]
            )
            tasks.append(task)
    if tasks: # 只有在需要生成语音时才执行 gather
        await asyncio.gather(*tasks)
    else:
        print("没有需要生成语音的片段。")


def play_voice_files(text_segments):
    """顺序播放生成的语音文件（注释掉实际播放，因为是在后端）"""
    for segment in text_segments:
        print(f"\n分析结果 (片段 {segment['index']}):")
        print(f"  原始标记: 【{segment['original_tag']}】")
        print(f"  中文文本: {segment['following_text']}")
        if segment['motion_text']:
             print(f"  动作文本: （{segment['motion_text']}）")
        if segment['japanese_text']:
             print(f"  日文文本: <{segment['japanese_text']}>")
        print(f"  预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
        if os.path.exists(segment['voice_file']):
            print(f"  对应语音: {os.path.basename(segment['voice_file'])}")
        else:
             # 如果日语文本为空，不应该期望有语音文件
             if segment['japanese_text']:
                print(f"  对应语音: (未生成或生成失败)")

# 语音合成（TTS）函数
async def text_to_speech(text, output_file):
    """生成单个语音文件"""
    # 确保文本不为空
    if not text or not text.strip():
        print(f"跳过为空的文本的语音生成: {output_file}")
        return
    try:
        await tts_engine.generate_voice(text, output_file, True)
        print(f"成功生成语音文件: {output_file}")
    except Exception as e:
        print(f"为文本 '{text}' 生成语音文件 {output_file} 时出错: {e}")


def create_responses(emotion_segments, user_message):
    # 构造多条回复
    responses = []
    total_parts = len(emotion_segments)
    for i, segment in enumerate(emotion_segments):
        # 提取文件名（去掉目录部分）
        audio_file_basename = os.path.basename(segment['voice_file']) if os.path.exists(segment['voice_file']) else None

        response = {
            "type": "reply",
            "emotion": segment['predicted'],
            "originalTag": segment['original_tag'],
            "message": segment['following_text'],
            "motionText": segment['motion_text'],
            # 只有当语音文件实际存在时才发送文件名
            "audioFile": audio_file_basename,
            "originalMessage": user_message,
            "isMultiPart": total_parts > 1,  # 只有当总部分数大于1时才是多部分
            "partIndex": i,                  # 当前部分索引 (从0开始)
            "totalParts": total_parts        # 总部分数
        }
        responses.append(response)

    return responses

async def process_ai_response(ai_response, user_message):
    """处理AI回复并分析情绪"""
    # 0. 清理临时语音文件夹中的所有旧的语音文件 (例如 .wav, .mp3, etc.)
    # 使用 tts_engine.format 获取正确的扩展名
    if os.path.exists(temp_voice_dir) and tts_engine.format:
        voice_pattern = os.path.join(temp_voice_dir, f"*.{tts_engine.format}")
        old_files = glob.glob(voice_pattern)
        print(f"清理旧语音文件 ({voice_pattern}): {len(old_files)} 个")
        for file in old_files:
            try:
                os.remove(file)
                # print(f"已删除: {file}")
            except Exception as e:
                print(f"删除文件 {file} 时出错: {e}")

    # 1. 打印原始回复
    print(f"\n{COLOR_AI}钦灵 (原始):{COLOR_RESET} {ai_response}")

    # 2. 分析情绪片段
    emotion_segments = analyze_emotions(ai_response)
    if not emotion_segments:
        print("警告: 未在AI响应中检测到有效的情绪/文本片段。")
        # 即使没有片段，也可能需要发送一个简单的文本回复
        # 或者返回一个空列表，让调用者决定如何处理
        return [] # 返回空列表

    # 3. 生成语音文件 (如果需要)
    print("\n开始生成语音文件...")
    await generate_voice_files(emotion_segments)
    print("语音文件生成完成 (或跳过)。")


    # 4. 构造消息包
    responses = create_responses(emotion_segments, user_message)

    # 5. 播放并显示分析结果 (现在只打印分析结果)
    print("\n--- AI 回复分析结果 ---")
    play_voice_files(emotion_segments)
    print("--- 分析结束 ---")


    return responses



async def handle_client(websocket):
    print("Python 服务: 新的连接建立，若钦灵长时间未回复，请刷新浏览器聊天界面重试")

    try:
        async for message in websocket:
            try:
                data = json.loads(message)
                print(f"\n>>> Python 收到消息: {data}")

                if data.get('type') == 'message':
                    user_message = data.get('content', '')
                    if not user_message:
                        print("收到空消息，已忽略。")
                        continue

                    print(f"{COLOR_USER}用户:{COLOR_RESET} {user_message}")
                    logger.log_conversation("用户", user_message)

                    # 调用 DeepSeek 获取回复
                    ai_response = deepseek.process_message(user_message)
                    logger.log_conversation("钦灵", ai_response)

                    # 处理 AI 回复、分析情绪、生成语音
                    responses = await process_ai_response(ai_response, user_message)

                    if responses:  # 确保 responses 是列表且不为空
                        print(f"准备发送 {len(responses)} 个消息片段到前端...")
                        for response in responses:
                            await websocket.send(json.dumps(response))
                            print(f"  已发送片段: index={response['partIndex']}, emotion={response['emotion']}, msg='{response['message'][:20]}...'")
                            await asyncio.sleep(0.1) # 短暂暂停，避免发送过快
                    else:
                        # 如果 process_ai_response 返回空列表 (例如，AI回复为空或无有效片段)
                        # 可以选择发送一个错误消息或默认回复
                        print("没有生成有效的回复片段。")
                        fallback_response = {
                            "type": "reply",
                            "message": "抱歉，我暂时无法处理您的请求。",
                            "emotion": "neutral", # 或者其他默认情绪
                            "originalMessage": user_message,
                            "isMultiPart": False,
                            "partIndex": 0,
                            "totalParts": 1
                        }
                        await websocket.send(json.dumps(fallback_response))
                        print("已发送回退消息。")

            except json.JSONDecodeError:
                print("错误: 收到了无效的 JSON 消息")
            except websockets.exceptions.ConnectionClosed:
                print("警告: 发送消息时连接已关闭")
                break # 退出内部循环
            except Exception as e:
                print(f"\n处理消息时发生内部错误: {str(e)}")
                # 尝试向客户端发送错误信息
                try:
                    await websocket.send(json.dumps({"type": "error", "message": f"服务器内部错误: {str(e)}"}))
                except Exception as send_err:
                    print(f"向客户端发送错误信息失败: {send_err}")

    except websockets.exceptions.ConnectionClosedOK:
        print("Python 服务: 连接正常关闭")
    except websockets.exceptions.ConnectionClosedError as e:
        print(f"Python 服务: 连接意外关闭 - {e}")
    except Exception as e:
        print(f"Python 服务: WebSocket 处理循环发生未知错误 - {e}")
    finally:
        print("Python 服务: 客户端处理结束.")


async def main():
    global node_process # 声明我们要修改全局变量
    print("main函数加载中...")

    # --- 注册退出处理函数 ---
    atexit.register(kill_node_process)
    # --- 结束注册 ---

    # --- 启动前端服务器 ---
    frontend_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'frontend'))
    server_js_path = os.path.join(frontend_dir, 'server.js')
    frontend_port = os.environ.get('FRONTEND_PORT', '3000') # 从环境变量或默认获取端口
    frontend_url = f"http://localhost:{frontend_port}"

    # 检查 server.js 是否存在
    if not os.path.isfile(server_js_path):
        print(f"错误：找不到前端脚本 {server_js_path}")
        print("请确保您在项目的根目录下运行此脚本，并且 'frontend' 文件夹及其内容存在。")
        return # 无法启动，退出

    # 构建 node 命令
    # 尝试直接使用 'node'，如果失败，提示用户检查环境
    node_command = ['node', os.path.basename(server_js_path)] # 在 frontend_dir 中运行

    print(f"\n尝试在目录 '{frontend_dir}' 中启动前端服务器:")
    print(f"  命令: {' '.join(node_command)}")

    try:
        # 使用 Popen 启动 Node.js 服务器作为一个子进程
        # cwd 参数确保 Node 服务器在正确的目录下运行，能找到它自己的文件
        # stdout=subprocess.PIPE, stderr=subprocess.PIPE 可以捕获输出，避免混淆 Python 输出
        # 但为了方便调试前端问题，这里暂时不捕获，让其直接输出到控制台
        node_process = subprocess.Popen(
            node_command,
            cwd=frontend_dir,
             # stdout=subprocess.PIPE, # 取消注释以捕获输出
             # stderr=subprocess.PIPE, # 取消注释以捕获输出
             # text=True,              # 如果捕获输出，则按文本解码
             creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0 # 在 Windows 上不创建新窗口
        )
        print(f"前端服务器进程已启动 (PID: {node_process.pid})。请查看控制台输出以了解其状态。")

        # 给前端服务器一点启动时间
        print("等待前端服务器启动 (3秒)...")
        time.sleep(3)

        # 检查 Node 进程是否意外退出了
        if node_process.poll() is not None:
             print(f"错误：前端服务器进程似乎已意外退出，退出码: {node_process.poll()}")
             print("请检查上面的前端服务器日志输出以获取详细信息。")
             kill_node_process() # 尝试清理 (虽然可能已经结束)
             return

        # --- 打开浏览器 ---
        print(f"尝试在默认浏览器中打开: {frontend_url}")
        try:
            webbrowser.open(frontend_url)
            print("浏览器已尝试打开。如果未自动打开，请手动访问该地址。")
        except Exception as browser_err:
             print(f"警告：自动打开浏览器失败: {browser_err}")
             print(f"请手动在浏览器中打开 {frontend_url}")


    except FileNotFoundError:
        print("\n错误：找不到 'node' 命令。")
        print("请确保 Node.js 已正确安装，并且 'node' 命令在系统的 PATH 环境变量中。")
        print("您可以从 https://nodejs.org/ 下载并安装 Node.js。")
        kill_node_process() # 确保清理 (虽然可能没启动)
        return # 无法继续
    except Exception as e:
        print(f"\n启动前端服务器或打开浏览器时发生未知错误: {e}")
        kill_node_process() # 尝试清理
        return # 无法继续
    # --- 前端启动和浏览器打开结束 ---


    # --- 启动后端 (WebSocket 服务器) ---
    # 确保端口是整数
    bind_port = int(os.environ.get("BACKEND_PORT", 8765))
    bind_addr = os.environ.get("BACKEND_BIND_ADDR", "0.0.0.0") # 保持 0.0.0.0 以便容器内外访问

    server = None # 初始化 server 变量
    try:
        print(f"\n准备启动 Python WebSocket 后端服务器...")
        # 使用 try...finally 确保即使 WebSocket 启动失败，也能尝试关闭 Node 进程
        server = await websockets.serve(
            handle_client,
            bind_addr,
            bind_port,
            ping_interval=20, # 添加 ping 保持连接
            ping_timeout=20,
            close_timeout=10,
            max_size=2**25,  # 32MB
            origins=None     # 允许所有来源 (开发时方便，生产环境应配置具体来源)
        )
        print(f"Python WebSocket 服务正在运行于 ws://{bind_addr}:{bind_port}")
        print("服务已就绪，等待客户端连接...")
        print("按 Ctrl+C 停止服务。")

        # 保持服务器运行，直到被中断或关闭
        await server.wait_closed()

    except OSError as e:
        if "address already in use" in str(e).lower():
             print(f"\n错误：端口 {bind_port} 已被占用。")
             print("请检查是否有其他程序正在使用该端口，或在 .env 文件中配置不同的 BACKEND_PORT。")
        else:
             print(f"\n启动 WebSocket 服务器时发生 OS 错误: {e}")
    except Exception as e:
        print(f"\n启动或运行 WebSocket 服务器时发生未知错误: {e}")
    finally:
        # 无论如何，确保在退出前尝试关闭服务器和子进程
        if server:
            server.close()
            await server.wait_closed() # 等待服务器完全关闭
            print("Python WebSocket 服务已关闭。")
        # atexit 注册的函数会在这里之后执行，用于关闭 node 进程
        # 但为了更明确，也可以在这里再调用一次
        kill_node_process()
        print("清理完成。")
    # --- 后端启动结束 ---


if __name__ == "__main__":
    print("程序启动！")
    try:
        # 运行主异步函数
        asyncio.run(main())
    except KeyboardInterrupt:
        # 用户按 Ctrl+C 时
        print("\n收到 Ctrl+C 信号，正在优雅关闭...")
        # asyncio.run 会自动处理异步任务的取消
        # atexit 注册的 kill_node_process 会被调用
    except Exception as e:
         print(f"\n程序顶层发生未捕获异常: {e}")
    finally:
        # 确保 node 进程在任何退出情况下都被处理
        # atexit 应该能处理，但这可以作为备用
        # kill_node_process() # 通常由 atexit 处理，避免重复调用
        print("程序退出。")