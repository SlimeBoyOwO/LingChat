import openai
from openai import OpenAI
import httpx  # 添加导入

class Adapter:
    def __init__(self, config):
        proxy = config.get('proxy')
        http_client = httpx.Client(proxy=proxy) if proxy else None
        
        self.client = OpenAI(
            api_key=config['api_key'],
            base_url=config.get('base_url', 'https://api.deepseek.com/v1'),
            http_client=http_client  # 添加代理支持
        )
    def create_chat_completion(self, data):
        data = {k: v for k, v in data.items() if k != 'logprobs'}
        response = self.client.chat.completions.create(**data)
        return {
            "id": response.id,
            "object": "chat.completion",
            "created": response.created,
            "model": response.model,
            "choices": [{
                "index": choice.index,
                "message": {
                    "role": choice.message.role,
                    "content": choice.message.content
                },
                "finish_reason": choice.finish_reason
            } for choice in response.choices],
            "usage": {
                "prompt_tokens": response.usage.prompt_tokens,
                "completion_tokens": response.usage.completion_tokens,
                "total_tokens": response.usage.total_tokens
            }
        }