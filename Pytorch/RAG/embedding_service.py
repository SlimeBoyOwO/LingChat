# file: embedding_service.py
import os
import torch
from flask import Flask, request, jsonify
from sentence_transformers import SentenceTransformer
import logging

# --- 配置 ---
# 模型名称，用于构建本地路径
EMBEDDING_MODEL_NAME = 'all-MiniLM-L6-v2'
# 模型所在的本地路径 (与下载脚本中的路径一致)
MODEL_PATH = os.path.join(os.path.dirname(__file__), 'embedding_models', EMBEDDING_MODEL_NAME)
# API服务监听的端口
SERVER_PORT = 5001

# --- 初始化 ---
# 设置日志
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

# 初始化Flask应用
app = Flask(__name__)

# 全局变量用于持有加载的模型
embedding_model = None

def load_model():
    """在服务启动时加载模型到内存中"""
    global embedding_model
    if not os.path.isdir(MODEL_PATH):
        logging.error("="*50)
        logging.error(f"错误：模型目录未找到: {MODEL_PATH}")
        logging.error("请先运行 'download_model.py' 脚本来下载模型。")
        logging.error("="*50)
        exit(1) # 找不到模型则退出服务

    try:
        device = 'cuda' if torch.cuda.is_available() else 'cpu'
        logging.info(f"正在从本地路径加载模型: {MODEL_PATH}")
        embedding_model = SentenceTransformer(MODEL_PATH, device=device)
        logging.info(f"模型 '{EMBEDDING_MODEL_NAME}' 加载成功，运行在设备: '{device}'")
    except Exception as e:
        logging.error(f"加载模型时发生严重错误: {e}")
        exit(1)

@app.route('/encode', methods=['POST'])
def encode_texts():
    """
    API端点，用于为文本列表生成嵌入向量。
    请求体 JSON 格式: {"texts": ["文本1", "文本2", ...]}
    响应体 JSON 格式: {"embeddings": [[...], [...], ...]}
    """
    if not request.is_json:
        return jsonify({"error": "请求必须是JSON格式"}), 400

    data = request.get_json()
    texts = data.get('texts')

    if not texts or not isinstance(texts, list):
        return jsonify({"error": "请求体中必须包含一个名为 'texts' 的非空列表"}), 400
    
    if embedding_model is None:
        return jsonify({"error": "模型尚未加载或加载失败，服务不可用"}), 503

    try:
        logging.info(f"收到 {len(texts)} 条文本的编码请求...")
        # 使用模型进行编码，并转换为Python列表以便JSON序列化
        embeddings = embedding_model.encode(texts, show_progress_bar=False).tolist()
        logging.info(f"成功生成 {len(embeddings)} 个嵌入向量。")
        return jsonify({"embeddings": embeddings})
    except Exception as e:
        logging.error(f"处理编码请求时出错: {e}")
        return jsonify({"error": "服务器内部错误"}), 500

@app.route('/health', methods=['GET'])
def health_check():
    """简单的健康检查端点"""
    if embedding_model:
        return jsonify({"status": "ok", "model_loaded": True}), 200
    else:
        return jsonify({"status": "error", "model_loaded": False}), 503

if __name__ == '__main__':
    load_model()
    logging.info(f"嵌入模型服务已启动，监听地址 http://0.0.0.0:{SERVER_PORT}")
    app.run(host='0.0.0.0', port=SERVER_PORT, debug=False)