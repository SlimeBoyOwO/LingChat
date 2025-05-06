import os
import json
import asyncio
import websockets
import glob
from deepseek import DeepSeek
import re
from predictor import EmotionClassifier  
from VitsTTS import VitsTTS              
from logger import Logger
from langDetect import LangDetect
import dotenv
import subprocess 
import webbrowser 
import time       
import atexit     
import sys        
import signal     

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

# --- 全局变量，用于持有 Node.js 进程 ---
node_process = None
# --- 标志，用于防止重复清理 ---
_cleanup_called = False

# --- 修改：终止 Node.js 进程，增加防止重复执行的逻辑 ---
def kill_node_process():
    """尝试终止正在运行的 Node.js 前端服务器进程，并防止重复执行"""
    global node_process, _cleanup_called
    if _cleanup_called:
        # print("清理函数已被调用，跳过。") # Debugging print
        return
    _cleanup_called = True # 标记已开始清理

    print("\n正在执行清理：尝试终止 Node.js 前端服务器...")
    if node_process and node_process.poll() is None: # 检查进程是否存在且仍在运行
        print(f"  正在终止 Node.js 进程 (PID: {node_process.pid})...")
        try:
            # 优先尝试 SIGTERM (更优雅)
            node_process.terminate() # 发送终止信号 (SIGTERM on POSIX, TerminateProcess on Windows)
            try:
                 # 等待最多 5 秒让其自行终止
                 node_process.wait(timeout=5)
                 print("  Node.js 前端服务器已响应终止信号并退出。")
            except subprocess.TimeoutExpired:
                 print("  警告：Node.js 进程未能在 5 秒内响应终止信号，尝试强制终止 (SIGKILL)...")
                 node_process.kill() # 强制终止 (SIGKILL on POSIX, TerminateProcess on Windows)
                 node_process.wait(timeout=2) # 短暂等待确认
                 print("  Node.js 前端服务器已被强制终止。")
            except Exception as wait_err: # 处理等待时可能出现的其他错误
                print(f"  等待 Node.js 进程终止时出错: {wait_err}. 尝试强制终止...")
                node_process.kill()
                node_process.wait(timeout=2)
                print("  Node.js 前端服务器已被强制终止 (后备)。")

        except ProcessLookupError:
             print("  错误：尝试终止时 Node.js 进程已不存在。")
        except Exception as e:
            print(f"  终止 Node.js 进程 (PID: {node_process.pid if node_process else '未知'}) 时发生意外错误: {e}")
    elif node_process and node_process.poll() is not None:
        print("  Node.js 进程已自行终止。")
    else:
        print("  没有找到正在运行的 Node.js 进程需要终止。")

    node_process = None # 重置变量
    print("Node.js 进程清理完成。")

# --- 新增：信号处理函数 ---
def handle_signal(sig, frame):
    """处理操作系统信号 (如 SIGTERM, SIGINT)"""
    print(f"\n收到信号 {signal.Signals(sig).name} ({sig})，开始优雅关闭...")
    kill_node_process()
    print("信号处理完成，正在强制退出 Python 脚本...")
    os._exit(0)

# --- 注册退出处理函数 (仍然有用，处理正常退出和未捕获异常) ---
atexit.register(kill_node_process)


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
        text_to_speak = segment["japanese_text"]
        if text_to_speak:
            task = text_to_speech(
                text_to_speak,
                segment["voice_file"]
            )
            tasks.append(task)
    if tasks:
        await asyncio.gather(*tasks)
    else:
        # print("没有需要生成语音的片段。") # 可以取消注释以查看此信息
        pass


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
             if segment['japanese_text']:
                print(f"  对应语音: (未生成或生成失败)")

async def text_to_speech(text, output_file):
    """生成单个语音文件"""
    if not text or not text.strip():
        return
    try:
        await tts_engine.generate_voice(text, output_file, True)
    except Exception as e:
        print(f"为文本 '{text}' 生成语音文件 {output_file} 时出错: {e}")


def create_responses(emotion_segments, user_message):
    responses = []
    total_parts = len(emotion_segments)
    for i, segment in enumerate(emotion_segments):
        audio_file_basename = os.path.basename(segment['voice_file']) if os.path.exists(segment['voice_file']) else None

        response = {
            "type": "reply",
            "emotion": segment['predicted'],
            "originalTag": segment['original_tag'],
            "message": segment['following_text'],
            "motionText": segment['motion_text'],
            "audioFile": audio_file_basename,
            "originalMessage": user_message,
            "isMultiPart": total_parts > 1,
            "partIndex": i,
            "totalParts": total_parts
        }
        responses.append(response)
    return responses

async def process_ai_response(ai_response, user_message):
    """处理AI回复并分析情绪"""
    if os.path.exists(temp_voice_dir) and tts_engine.format:
        voice_pattern = os.path.join(temp_voice_dir, f"*.{tts_engine.format}")
        old_files = glob.glob(voice_pattern)
        for file in old_files:
            try:
                os.remove(file)
            except Exception as e:
                print(f"删除文件 {file} 时出错: {e}")

    print(f"\n{COLOR_AI}钦灵 (原始):{COLOR_RESET} {ai_response}")

    emotion_segments = analyze_emotions(ai_response)
    if not emotion_segments:
        print("警告: 未在AI响应中检测到有效的情绪/文本片段。请检查.env文件中apikey是否填写无误，或登录网站检查apikey是否还有余额")
        return []

    await generate_voice_files(emotion_segments)

    responses = create_responses(emotion_segments, user_message)

    print("\n--- AI 回复分析结果 ---")
    play_voice_files(emotion_segments)
    print("--- 分析结束 ---")

    return responses


async def handle_client(websocket):
    print("Python 服务: 新的连接建立")
    try:
        async for message in websocket:
            try:
                data = json.loads(message)

                if data.get('type') == 'message':
                    user_message = data.get('content', '')
                    if not user_message:
                        print("收到空消息，已忽略。")
                        continue

                    print(f"{COLOR_USER}用户:{COLOR_RESET} {user_message}")
                    logger.log_conversation("用户", user_message)

                    ai_response = deepseek.process_message(user_message)
                    logger.log_conversation("钦灵", ai_response)

                    responses = await process_ai_response(ai_response, user_message)

                    if responses:
                        print(f"准备发送 {len(responses)} 个消息片段到前端...")
                        for response in responses:
                            await websocket.send(json.dumps(response))
                            await asyncio.sleep(0.1)
                    else:
                        print("没有生成有效的回复片段。发送回退消息。")
                        fallback_response = {
                            "type": "reply",
                            "message": "抱歉，我暂时无法处理您的请求。",
                            "emotion": "害羞",
                            "originalMessage": user_message,
                            "isMultiPart": False, "partIndex": 0, "totalParts": 1
                        }
                        await websocket.send(json.dumps(fallback_response))

            except json.JSONDecodeError:
                print("错误: 收到了无效的 JSON 消息")
            except websockets.exceptions.ConnectionClosed:
                print("警告: 发送消息时连接已关闭")
                break
            except Exception as e:
                print(f"\n处理消息时发生内部错误: {str(e)}")
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
    global node_process, _cleanup_called # 声明我们要修改全局变量
    print("main函数加载中...")
    _cleanup_called = False

    signals_to_catch = (signal.SIGINT, signal.SIGTERM)
    if sys.platform != "win32":
        signals_to_catch += (signal.SIGHUP,)
    else:
        signals_to_catch += (signal.SIGBREAK,)

    for sig in signals_to_catch:
        try:
            signal.signal(sig, handle_signal)
        except (OSError, ValueError, AttributeError) as e:
            print(f"警告: 无法注册信号 {sig} 的处理器: {e}")
    # --- 信号注册结束 ---


    # --- 启动前端服务器 ---
    frontend_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'frontend', 'server'))
    server_js_path = os.path.join(frontend_dir, "app.js")
    frontend_port = os.environ.get('FRONTEND_PORT', '3000')
    frontend_url = f"http://localhost:{frontend_port}"

    if not os.path.isfile(server_js_path):
        print(f"错误：找不到前端脚本 {server_js_path}")
        print("请确保您在项目的根目录下运行此脚本，并且 'frontend' 文件夹及其内容存在。")
        return

    node_command = ['node', os.path.basename(server_js_path)]
    print(f"\n尝试在目录 '{frontend_dir}' 中启动前端服务器:")
    print(f"  命令: {' '.join(node_command)}")

    try:
        creationflags = subprocess.CREATE_NEW_PROCESS_GROUP if sys.platform == 'win32' else 0
        start_new_session = True if sys.platform != 'win32' else False

        node_process = subprocess.Popen(
            node_command,
            cwd=frontend_dir,
            creationflags=creationflags,
            start_new_session=start_new_session
        )
        print(f"前端服务器进程已启动 (PID: {node_process.pid})。")

        print("等待前端服务器启动 (3秒)...")
        time.sleep(3)

        if node_process.poll() is not None:
             print(f"错误：前端服务器进程似乎已意外退出，退出码: {node_process.poll()}")
             print("请检查上面的前端服务器日志输出以获取详细信息。")
             return

        print(f"尝试在默认浏览器中打开: {frontend_url}")
        try:
            webbrowser.open(frontend_url)
            print("浏览器已尝试打开。如果未自动打开，请手动访问该地址。")
        except Exception as browser_err:
             print(f"警告：自动打开浏览器失败: {browser_err}")
             print(f"请手动在浏览器中打开 {frontend_url}")

    except FileNotFoundError:
        print("\n错误：找不到 'node' 命令。请确保 Node.js 已安装并添加到 PATH。")
        return
    except Exception as e:
        print(f"\n启动前端服务器或打开浏览器时发生未知错误: {e}")
        return
    # --- 前端启动结束 ---

    # --- 启动后端 (WebSocket 服务器) ---
    server = None
    websocket_running = False
    try:
        bind_port = int(os.environ.get("BACKEND_PORT", 8765))
        bind_addr = os.environ.get("BACKEND_BIND_ADDR", "0.0.0.0")

        print(f"\n准备启动 Python WebSocket 后端服务器...")
        server = await websockets.serve(
            handle_client, bind_addr, bind_port,
            ping_interval=20, ping_timeout=20, close_timeout=10,
            max_size=2**25, origins=None
        )
        websocket_running = True
        print(f"Python WebSocket 服务正在运行于 ws://{bind_addr}:{bind_port}")
        print("服务已就绪，等待客户端连接...")
        print("按 Ctrl+C 或关闭终端以停止服务。")
        await asyncio.Future()

    except OSError as e:
        if "address already in use" in str(e).lower():
             print(f"\n错误：端口 {bind_port} 已被占用。")
             print("请检查是否有其他程序正在使用该端口，或在 .env 文件中配置不同的 BACKEND_PORT。")
        else:
             print(f"\n启动 WebSocket 服务器时发生 OS 错误: {e}")
    except Exception as e:
        print(f"\n启动或运行 WebSocket 服务器时发生未知错误: {e}")
    finally:
        print("\n进入 main 函数的 finally 块...")
        if server and websocket_running:
            print("正在关闭 Python WebSocket 服务...")
            server.close()
            try:
                await asyncio.wait_for(server.wait_closed(), timeout=5.0)
                print("Python WebSocket 服务已关闭。")
            except asyncio.TimeoutError:
                print("警告: 等待 WebSocket 服务关闭超时。")
            except Exception as close_err:
                print(f"关闭 WebSocket 服务时出错: {close_err}")
        print("main 函数 finally 块执行完毕。")


if __name__ == "__main__":
    print("程序启动！")
    loop = None
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n收到 KeyboardInterrupt (可能是 SIGINT 信号)... 清理应由信号处理器或 atexit 处理。")
    except SystemExit as e:
        print(f"程序通过 SystemExit({e.code}) 退出。")
    except Exception as e:
         print(f"\n程序顶层发生未捕获异常: {e}")
    finally:
        print("程序最终退出。")