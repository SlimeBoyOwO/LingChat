
import requests
import time

class Adapter:
    def __init__(self, config):
        self.base_url = config['base_url']
        
    def create_chat_completion(self, data):
        models = data['model']
        models = models.split("-")
        model = models[1]
        messages = data['messages']
        temperature = data.get('temperature', 0.7)
        top_p = data.get('top_p', 0.9)
        response = requests.post(
            f"{self.base_url}/api/generate",
            json={
                "model": model,
                "prompt": self._format_prompt(messages),
                "stream": False,
                "options": {"temperature": temperature, "top_p": top_p}
            }
        )
        if response.status_code != 200:
            raise RuntimeError(f"Ollama error: {response.text}")
            
        response_data = response.json()
        
        return {
            "id": f"ollama-{int(time.time())}",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": response_data['response']
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": response_data.get('prompt_eval_count', 0),
                "completion_tokens": response_data.get('eval_count', 0),
                "total_tokens": response_data.get('prompt_eval_count', 0) + response_data.get('eval_count', 0)
            }
        }
    
    def _format_prompt(self, messages):
        return "\n".join(f"{msg['role']}: {msg['content']}" for msg in messages)