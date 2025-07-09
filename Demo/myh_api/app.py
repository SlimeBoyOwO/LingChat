from flask import Flask, request, jsonify
from adapters import get_adapter
import time

app = Flask(__name__)
@app.route('/')
def health_check():
    return jsonify({"status": "ok", "message": "API is running"})
@app.route('/chat/completions', methods=['POST'])
def unified_api():
    data = request.json
    model = data.get('model', '')
    
    try:
        adapter = get_adapter(model)
        start_time = time.time()
        response = adapter.create_chat_completion(data)
        duration = time.time() - start_time
        app.logger.info(f"API响应完成 - 模型: {model}, 耗时: {duration:.2f}s")
        return jsonify(response)
    except Exception as e:
        app.logger.error(f"API请求失败: {str(e)}")
        return jsonify({
            "error": {
                "message": str(e),
                "type": "invalid_request_error"
            }
        }), 400

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8090, debug=True)