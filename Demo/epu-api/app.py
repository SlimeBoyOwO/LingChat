from flask import Flask, request, jsonify
from adapters import get_adapter
import time
import yaml
import os
import re

app = Flask(__name__)

config_path = os.path.join(os.path.dirname(__file__), 'config.yaml')
with open(config_path, encoding='utf-8') as f:
    CONFIG = yaml.safe_load(f)

MY_API_KEYS = set()
for key in CONFIG.get('my_api_keys', []):
    if isinstance(key, str):
        MY_API_KEYS.add(key.strip()) 
    elif isinstance(key, (int, float)):
        MY_API_KEYS.add(str(key).strip())

@app.route('/')
def health_check():
    return jsonify({"status": "ok", "message": "API is running"})

@app.route('/v1/chat/completions', methods=['POST'])
@app.route('/chat/completions', methods=['POST'])
def unified_api():

    apikey = None
    
    sources = [
        request.headers.get('X-API-KEY'),
        request.headers.get('Authorization'),
        request.args.get('apikey'),
        (request.get_json(silent=True) or {}).get('apikey')
    ]
    
    for source in sources:
        if source and isinstance(source, str):
            if source.startswith("Bearer "):
                apikey = source[7:].strip()
            else:
                apikey = source.strip()
            break
    
    if apikey:
        masked_apikey = apikey[:4] + '*' * (len(apikey) - 8) + apikey[-4:] if len(apikey) > 8 else "***"
        app.logger.info(f"Received API key: {masked_apikey}")
    else:
        app.logger.warning("No API key provided")
    
    if not apikey:
        return jsonify({
            "error": {
                "message": "Missing API key. Please provide a valid API key.",
                "type": "authentication_error"
            }
        }), 401
    
    if apikey not in MY_API_KEYS:
        return jsonify({
            "error": {
                "message": f"Authentication failed. The provided API key ({masked_apikey}) is invalid.",
                "type": "authentication_error",
                "suggestion": "Check your config.yaml file for valid API keys"
            }
        }), 401

    data = request.get_json()
    if data is None:
        return jsonify({
            "error": {
                "message": "Invalid or missing JSON in request body.",
                "type": "invalid_request_error"
            }
        }), 400

    keys_to_remove = ['api_key', 'apikey', 'api-key', 'key', 'access_key']
    for key in keys_to_remove:
        data.pop(key, None)
    
    model = data.get('model', '')
    try:
        adapter = get_adapter(model)
        app.logger.info(f"Using adapter for model: {model}")
        start_time = time.time()
        response = adapter.create_chat_completion(data)
        duration = time.time() - start_time
        app.logger.info(f"API response completed - Model: {model}, Duration: {duration:.2f}s")
        return jsonify(response)
    except Exception as e:
        app.logger.error(f"API request failed: {str(e)}")
        return jsonify({
            "error": {
                "message": str(e),
                "type": "invalid_request_error"
            }
        }), 400
    
if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8090, debug=True)