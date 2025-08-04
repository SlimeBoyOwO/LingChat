import json
import os
import socket
from fastapi import WebSocket, WebSocketDisconnect
from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
import time

class WebSocketManager:
    async def handle_websocket(self, websocket: WebSocket):
        await websocket.accept()
        # 全加一遍标签免得报错websocket连接错误
        disconnected = False
        try:
            while not disconnected:
                try:
                    message = await websocket.receive()
                    # 首先检查是否是断开消息
                    if message.get('type') == 'websocket.disconnect':
                        logger.info(f"客户端断开连接，代码: {message.get('code')}")
                        disconnected = True
                    else:
                        data = json.loads(message["text"])

                        if data.get('type') == 'ping':
                            await websocket.send_json({"type": "pong"})
                        elif data.get('type') == 'message':
                            print(message)
                            start_time = time.time()
                            try:
                                if os.environ.get("USE_STREAM", "False").lower() == "true":
                                    responses = await service_manager.ai_service.process_message_stream_compat(data.get('content', ''))
                                else:
                                    responses = await service_manager.ai_service.process_message(data.get('content', ''))

                                for response in responses:
                                    await websocket.send_json(response)
                                stop_time = time.time()
                                logger.debug(f"此次对话生成耗时 {stop_time - start_time} 秒。")

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
                    logger.info("客户端断开连接")
                    disconnected = True
                except ConnectionResetError:
                    logger.info("客户端强制关闭连接")
                    disconnected = True
                except socket.error as e:
                    if e.errno == 10054:
                        logger.info("远程主机强迫关闭了一个现有的连接")
                    else:
                        logger.error(f"Socket错误: {e}")
                    disconnected = True
                except Exception as e:
                    logger.error(f"WebSocket连接错误: {e}")
                    try:
                        await websocket.close(code=1011, reason=f"内部错误: {str(e)}")
                    except:
                        pass
                    disconnected = True
        finally:
            logger.info("WebSocket连接已关闭")

ws_manager = WebSocketManager()

async def websocket_endpoint(websocket: WebSocket):
    await ws_manager.handle_websocket(websocket)