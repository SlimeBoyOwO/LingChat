import requests
import time
import logging

class Adapter:
    def __init__(self, config):
        self.base_url = config['base_url']

    def create_chat_completion(self, data):
        
        model_name = data['model']
        model = model_name[len('ollama-'):] if model_name.startswith('ollama-') else model_name
        payload = {
            "model": model,
            "messages": data['messages'],
            "stream": False,
            "options": {
                "temperature": data.get('temperature', 0.7),
                "top_p": data.get('top_p', 0.9)
            }
        }

        try:
            resp = requests.post(f"{self.base_url}/api/chat", json=payload)
            resp.raise_for_status()
            res = resp.json()
        except Exception as e:
            logging.error(f"Ollama API error: {e}")
            return {
                "error": {
                    "message": f"Ollama API error: {e}",
                    "type": "api_error"
                }
            }

        msg = res.get('message', {})
        content = msg.get('content', '')
        if not content:
            logging.error(f"Invalid Ollama response: {res}")
            return {
                "error": {
                    "message": "Invalid response format from Ollama",
                    "type": "invalid_response",
                    "response": res
                }
            }

        return {
            "id": f"ollama-{int(time.time())}",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": res.get('done_reason', 'stop')
            }],
            "usage": {
                "prompt_tokens": res.get('prompt_eval_count', 0),
                "completion_tokens": res.get('eval_count', 0),
                "total_tokens": res.get('prompt_eval_count', 0) + res.get('eval_count', 0)
            }
        }
