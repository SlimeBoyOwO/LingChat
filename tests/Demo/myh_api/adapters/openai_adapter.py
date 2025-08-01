import openai

class Adapter:
    def __init__(self, config):
        openai.api_key = config['api_key']
        self.client = openai.OpenAI(base_url=config.get('base_url', 'https://api.openai.com/v1'))

    def create_chat_completion(self, data):
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