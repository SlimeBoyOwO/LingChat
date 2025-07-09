import openai

class Adapter:
    def __init__(self, config):
        # 仅使用config中的API key
        self.client = openai.OpenAI(
            api_key=config['api_key'],
            base_url=config.get('base_url', 'https://api.openai.com/v1')
        )

    def create_chat_completion(self, data):
        # 确保不使用请求中的API key
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
            "usage": dict(response.usage)
        }
