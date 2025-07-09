import dashscope
from dashscope import Generation
import time

class Adapter:
    def __init__(self, config):
        # 仅使用config中的API key
        dashscope.api_key = config['api_key']
        
    def create_chat_completion(self, data):
        # 确保不使用请求中的API key
        response = Generation.call(
            model=data['model'],
            messages=data['messages'],
            temperature=data.get('temperature', 0.7),
            top_p=data.get('top_p', 0.8)
        )
        
        if response.status_code != 200:
            raise RuntimeError(f"Qwen error: {response}")
            
        return {
            "id": response.request_id,
            "object": "chat.completion",
            "created": int(time.time()),
            "model": data['model'],
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": response.output.choices[0]['message']['content']
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": response.usage.input_tokens,
                "completion_tokens": response.usage.output_tokens,
                "total_tokens": response.usage.input_tokens + response.usage.output_tokens
            }
        }
