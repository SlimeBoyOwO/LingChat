import asyncio
import websockets
import json
import os

async def handle_client(websocket, path):
    print("Python 服务: 新的连接建立")
    
    try:
        async for message in websocket:
            # 接收来自 Node.js 的消息
            data = json.loads(message)
            print(f"Python 收到消息: {data}")
            
            # 处理消息
            if data.get('type') == 'message':
                user_message = data.get('content', '')
                
                # 业务逻辑处理 - 现在返回多条消息
                if "你好" in user_message:
                    reply_messages = [
                        "你好呀！",
                        "很高兴见到你！",
                        "今天有什么我可以帮你的吗？"
                    ]
                    audio_files = ["happy.wav", None, None]  # 每条消息对应的音频文件
                elif "笨蛋" in user_message:
                    reply_messages = [
                        "你才笨蛋！",
                        "哼！不想理你了！",
                        "除非你道歉！"
                    ]
                    audio_files = ["angry.wav", "angry.wav", None]
                else:
                    reply_messages = [
                        f"我收到了: {user_message}",
                        "这是第二条回复",
                        "这是最后一条回复"
                    ]
                    audio_files = [None, None, None]
                
                # 检查音频文件是否存在
                for i, audio_file in enumerate(audio_files):
                    if audio_file:
                        audio_path = os.path.join("public", "audio", audio_file)
                        if not os.path.exists(audio_path):
                            print(f"警告: 音频文件 {audio_file} 不存在")
                            audio_files[i] = None
                
                # 构造多条回复
                responses = []
                for msg, audio in zip(reply_messages, audio_files):
                    response = {
                        "type": "reply",
                        "message": msg,
                        "audioFile": audio,
                        "originalMessage": user_message,
                        "isMultiPart": True,  # 标记是多部分消息
                        "partIndex": len(responses),  # 当前部分索引
                        "totalParts": len(reply_messages)  # 总部分数
                    }
                    responses.append(response)
                
                # 发送所有回复
                for response in responses:
                    await websocket.send(json.dumps(response))
                    await asyncio.sleep(0.1)  # 添加小的延迟确保顺序
                
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