import json
import time
import websocket
from urllib.parse import urlencode

class Adapter:
    def __init__(self, config):
        # 仅使用config中的凭证
        self.config = config
        
    def create_chat_completion(self, data):
        # 确保不使用请求中的API key
        messages = data['messages']
        query = messages[-1]['content']
        history = [{'role': msg['role'], 'content': msg['content']} 
                  for msg in messages[:-1]]
        
        ws_url = self._build_ws_url()
        ws = websocket.create_connection(ws_url)
        
        request = {
            "header": {"app_id": self.config['app_id']},
            "parameter": {"chat": {"domain": "generalv3"}},
            "payload": {"message": {"text": history + [{"role": "user", "content": query}]}}
        }
        
        ws.send(json.dumps(request))
        result = ''
        while True:
            response = json.loads(ws.recv()))
            if response['header']['code'] != 0:
                raise RuntimeError(f"Spark error: {response['header']}")
            result += response['payload']['choices']['text'][0]['content']
            if response['header']['status'] == 2:
                break
        ws.close()
        
        return {
            "id": f"spark-{int(time.time())}",
            "object": "chat.completion",
            "created": int(time.time()),
            "model": data['model'],
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": result},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 0, "completion_tokens": 0}
        }
    
    def _build_ws_url(self):
        params = {
            "appid": self.config['app_id'],
            "timestamp": str(int(time.time())),
        }
        return f"wss://spark-api.xf-yun.com/v3.1/chat?{urlencode(params)}"
