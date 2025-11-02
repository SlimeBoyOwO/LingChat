import asyncio
import json
from fastapi import WebSocket, WebSocketDisconnect
from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.core.messaging.broker import message_broker
import traceback

class WebSocketManager:
    async def handle_websocket(self, websocket: WebSocket, client_id: str):
        await websocket.accept()
        try:
            # 启动独立的发送任务
            send_task = asyncio.create_task(
                self._send_messages(websocket, client_id)
            )
            
            # 处理接收的消息
            async for message in self._receive_messages(websocket):
                # 首先检查是否是断开消息
                if message.get('type') == 'websocket.disconnect':
                    logger.info(f"客户端断开连接，代码: {message.get('code')}")
                    disconnected = True
                    return
                
                if message.get('type') == 'ping':
                    await websocket.send_json({"type": "pong"})
                elif message.get('type') == 'message':
                    logger.info("来自客户端的消息：" + str(message))
                    # 将消息转发给 ai_service
                    ai_service = service_manager.ai_service
                    if ai_service is not None:
                        user_message = message.get('content', '')
                        if user_message == "/开始剧本":
                            asyncio.create_task(
                                ai_service.start_script()
                            )
                            logger.info("开始进行剧本模式")
                        else:
                            if ai_service.scripts_manager.is_running:
                                asyncio.create_task(
                                    message_broker.enqueue_ai_script_message(client_id, user_message)
                                )
                            else:
                                asyncio.create_task(
                                    message_broker.enqueue_ai_message(client_id, user_message)
                                )
                    else:
                        logger.error("尚未初始化，请刷新网页！")
                        #TODO: 向前端提示或者之后做个刷新系统

        except Exception as e:
            logger.error(f"WebSocket连接错误: {e}")
            try:
                await websocket.close(code=1011, reason=f"内部错误: {str(e)}")
            except:
                pass
            disconnected = True
            traceback.print_exc()
        finally:
            send_task.cancel()
            logger.info(f"客户端 {client_id} 断开连接")

    async def _receive_messages(self, websocket: WebSocket):
        """接收消息的通用逻辑"""
        while True:
            try:
                message = await websocket.receive()
                if message.get('type') == 'websocket.disconnect':
                    break
                yield json.loads(message["text"])
            except (WebSocketDisconnect, ConnectionResetError):
                break

    async def _send_messages(self, websocket: WebSocket, client_id: str):
        """从消息队列中获取并发送消息"""
        async for message in message_broker.subscribe(client_id):
            if message:
                logger.info("存在信息" + str(message))
            try:
                await websocket.send_json(message)
            except Exception as e:
                logger.error(f"发送消息失败: {e}")
                break

ws_manager = WebSocketManager()

async def websocket_endpoint(websocket: WebSocket):
    await ws_manager.handle_websocket(websocket, "1")

# TODO 目前，先锁定 client_id 为 1 