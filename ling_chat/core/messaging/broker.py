import asyncio
from typing import Dict, AsyncGenerator

class MessageBroker:
    def __init__(self):
        self.queues: Dict[str, asyncio.Queue] = {}

    async def publish(self, client_id: str, message: dict):
        if client_id not in self.queues:
            self.queues[client_id] = asyncio.Queue()
        await self.queues[client_id].put(message)

    async def subscribe(self, client_id: str) -> AsyncGenerator[dict, None]:
        if client_id not in self.queues:
            self.queues[client_id] = asyncio.Queue()
        while True:
            yield await self.queues[client_id].get()

# 单例模式
message_broker = MessageBroker()