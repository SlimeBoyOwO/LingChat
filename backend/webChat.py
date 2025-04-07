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

logger = Logger()
deepseek = DeepSeek()
emotion_classifier = EmotionClassifier()
tts_engine = VitsTTS(
    api_url="http://192.168.31.228:23456/voice/vits",
    speaker_id=4,
    lang="ja"  # 根据角色设定调整
)
temp_voice_dir = "../public/audio"
os.makedirs(temp_voice_dir, exist_ok=True)

# ANSI 颜色代码
COLOR_USER = "\033[92m"  # 绿色
COLOR_AI = "\033[96m"    # 青蓝色
COLOR_EMOTION = "\033[93m" # 黄色（用于情绪显示）
COLOR_RESET = "\033[0m"  # 重置颜色

def analyze_emotions(text):
    """分析文本中每个【】标记的情绪，并额外提取日语部分"""
    # 提取所有【】内的情绪词及其后续文本
    emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)
    
    results = []
    for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):

        # 统一处理中文/英文括号（根据实际数据选择一种）
        following_text = following_text.replace('(', '（').replace(')', '）')  # 英文转中文括号（可选）
        
        # 提取日语部分（#...#）
        japanese_match = re.search(r'#(.*?)#', following_text)
        japanese_text = japanese_match.group(1) if japanese_match else ""
        
        # 提取动作部分（（...））
        motion_match = re.search(r'（(.*?)）', following_text)  # 确保是中文括号
        motion_text = motion_match.group(1) if motion_match else ""
        
        # 清理后的文本
        cleaned_text = re.sub(r'#.*?#|（.*?）', '', following_text).strip()
        japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

        if not following_text and not japanese_text and not motion_text:  # 跳过完全空的文本
            continue
        
        # 对情绪标签单独预测
        predicted = emotion_classifier.predict(emotion_tag)
        
        results.append({
            "index": i,
            "original_tag": emotion_tag,
            "following_text": cleaned_text,
            "motion_text": motion_text,
            "japanese_text": japanese_text,
            "predicted": predicted["label"],
            "confidence": predicted["confidence"],
            "voice_file": os.path.join(temp_voice_dir, f"part_{i}.{tts_engine.format}")
        })
    
    return results

async def generate_voice_files(text_segments):
    """异步生成所有语音文件"""
    tasks = []
    for segment in text_segments:
        if segment["japanese_text"]:  # 确保有实际内容
            task = text_to_speech(
                segment["japanese_text"], 
                segment["voice_file"]
            )
            tasks.append(task)
    await asyncio.gather(*tasks)

def play_voice_files(text_segments):
    """顺序播放生成的语音文件"""
    for segment in text_segments:
        if os.path.exists(segment["voice_file"]):
            print(f"\n播放: 【{segment['original_tag']}】{segment['following_text']}")
            print(f"\n日文翻译: {segment['japanese_text']}")
            print(f"预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            # 使用 pydub 播放音频，避免文件占用问题
            

# 语音合成（TTS）函数
async def text_to_speech(text, output_file):
    """生成单个语音文件"""
    await tts_engine.generate_voice(text, output_file, True)

def create_responses(emotion_segments, user_message):
    # 构造多条回复
    responses = []
    for segment in emotion_segments:
        # 提取文件名（去掉目录部分）
        audio_file = os.path.basename(segment['voice_file'])
        
        response = {
            "type": "reply",
            "emotion": segment['predicted'],
            "originalTag": segment['original_tag'],
            "message": segment['following_text'],
            "motionText": segment['motion_text'],
            "audioFile": audio_file,
            "originalMessage": user_message,
            "isMultiPart": True,  # 标记是多部分消息
            "partIndex": len(responses),  # 当前部分索引
            "totalParts": len(emotion_segments)  # 总部分数
        }
        responses.append(response)
    
    return responses

async def process_ai_response(ai_response, user_message):
    """处理AI回复并分析情绪"""
    # 0. 清理临时语音文件夹中的所有.wav文件
    if os.path.exists(temp_voice_dir):
        wav_files = glob.glob(os.path.join(temp_voice_dir, "*.wav"))
        for file in wav_files:
            try:
                os.remove(file)
            except Exception as e:
                print(f"删除文件 {file} 时出错: {e}")

    # 1. 打印原始回复
    print(f"\n{COLOR_AI}钦灵:{COLOR_RESET} {ai_response}")
    
    # 2. 分析情绪片段
    emotion_segments = analyze_emotions(ai_response)
    if not emotion_segments:
        print("未检测到有效情绪片段")
        return
    
    # 3. 生成语音文件
    print("\n生成语音文件中...")
    await generate_voice_files(emotion_segments)

    # 4. 构造消息包
    responses = create_responses(emotion_segments, user_message)

    # 5. 播放并显示分析结果
    print("\n语音分析结果:")
    play_voice_files(emotion_segments)

    return responses
    
    

async def handle_client(websocket, path):
    print("Python 服务: 新的连接建立")
    
    try:
        async for message in websocket:
            data = json.loads(message)
            print(f"Python 收到消息: {data}")
            
            if data.get('type') == 'message':
                user_message = data.get('content', '')
                logger.log_conversation("用户", user_message)
                ai_response = deepseek.process_message(user_message)
                logger.log_conversation("钦灵", ai_response)
                
                try:
                    responses = await process_ai_response(ai_response, user_message)
                    if responses:  # 确保responses不是None
                        for response in responses:
                            await websocket.send(json.dumps(response))
                            await asyncio.sleep(0.1)
                except Exception as e:
                    print(f"\n处理AI响应时出错: {str(e)}")
                    await websocket.send(json.dumps({"error": str(e)}))
                
    except websockets.exceptions.ConnectionClosedOK:
        print("Python 服务: 连接正常关闭")
    except Exception as e:
        print(f"Python 服务: 发生错误 - {e}")

async def main():
    # 启动 WebSocket 服务器
    server = await websockets.serve(
        handle_client, 
        "localhost", 
        8765,
        ping_interval=None
    )
    
    print("Python WebSocket 服务运行在 ws://localhost:8765")
    await server.wait_closed()

if __name__ == "__main__":
    asyncio.get_event_loop().run_until_complete(main())