# file: download_model.py
import os
from sentence_transformers import SentenceTransformer
import logging

# --- 配置 ---
# 模型名称，与RAG系统中使用的名称保持一致
EMBEDDING_MODEL_NAME = 'all-MiniLM-L6-v2'
# 定义模型将要保存的本地路径 (例如，在项目根目录下的 'models' 文件夹)
# 这可以根据您的项目结构进行调整
SAVE_DIRECTORY = os.path.join(os.path.dirname(__file__), 'embedding_models')
TARGET_MODEL_PATH = os.path.join(SAVE_DIRECTORY, EMBEDDING_MODEL_NAME)

# 设置简单的日志记录
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

def download_embedding_model():
    """
    从Hugging Face Hub下载并保存Sentence Transformer模型到本地指定目录。
    如果模型目录已存在，则跳过下载。
    """
    if os.path.exists(TARGET_MODEL_PATH) and os.listdir(TARGET_MODEL_PATH):
        logging.info(f"模型 '{EMBEDDING_MODEL_NAME}' 似乎已存在于: {TARGET_MODEL_PATH}")
        logging.info("跳过下载。如果需要重新下载，请先删除该目录。")
        return

    logging.info(f"开始下载模型: '{EMBEDDING_MODEL_NAME}'")
    logging.info("这可能需要一些时间，取决于您的网络连接...")

    try:
        # 1. 从Hugging Face加载模型 (这将触发下载到缓存)
        model = SentenceTransformer(EMBEDDING_MODEL_NAME)

        # 2. 将模型从缓存保存到我们的目标目录
        logging.info(f"模型下载完成，正在将其保存到: {TARGET_MODEL_PATH}")
        os.makedirs(TARGET_MODEL_PATH, exist_ok=True)
        model.save(TARGET_MODEL_PATH)

        logging.info("="*50)
        logging.info(f"模型已成功下载并保存！")
        logging.info(f"路径: {os.path.abspath(TARGET_MODEL_PATH)}")
        logging.info("="*50)

    except Exception as e:
        logging.error(f"下载或保存模型时发生错误: {e}")
        logging.error("请检查您的网络连接以及是否安装了 'sentence-transformers' 库 (pip install sentence-transformers)。")

if __name__ == "__main__":
    download_embedding_model()