import json
import asyncio
from fastapi import WebSocket, WebSocketDisconnect
from py_ling_chat.core.logger import logger
from py_ling_chat.core.service_manager import service_manager

class WebSocketManager:
    async def handle_websocket(self, websocket: WebSocket):
        await websocket.accept()
        try:
            while True:
                message = await websocket.receive()
                # 首先检查是否是断开消息
                if message.get('type') == 'websocket.disconnect':
                    logger.info(f"客户端断开连接，代码: {message.get('code')}")
                else:
                    data = json.loads(message["text"])

                    if data.get('type') == 'ping':
                        await websocket.send_json({"type": "pong"})
                    elif data.get('type') == 'message':
                        print(message)
                        # 添加错误处理
                        try:
                            responses = await service_manager.ai_service.process_message(data.get('content', ''))
                            
                            for response in responses:
                                await websocket.send_json(response)

                        except Exception as e:
                            logger.error(f"处理消息时发生异常: {e}")
                            try:
                                # 发送错误响应
                                await websocket.send_json({
                                    "type": "reply",
                                    "emotion": "sad",
                                    "originalTag": "错误",
                                    "message": f"处理消息时出错: {str(e)}",
                                    "motionText": "困惑",
                                    "audioFile": None,
                                    "originalMessage": data.get('content', ''),
                                    "isMultiPart": False,
                                    "partIndex": 0,
                                    "totalParts": 1
                                })
                            except:
                                logger.error("无法发送错误响应")      
        except WebSocketDisconnect:
            print("客户端断开连接")
        except Exception as e:
            logger.error(f"WebSocket连接错误: {e}")

async def websocket_endpoint(websocket: WebSocket):
    await WebSocketManager().handle_websocket(websocket)