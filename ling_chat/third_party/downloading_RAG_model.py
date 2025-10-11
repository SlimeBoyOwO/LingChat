import os
import sys
from ling_chat.utils.runtime_path import third_party_path
from ling_chat.core.logger import logger

def download_embedding_model(use_mirror=False):
    """
    此脚本用于下载RAG系统所需的'all-MiniLM-L6-v2'嵌入模型，
    并将其保存到指定的本地目录中，以便RAG.py可以离线加载。
    """
    model_name = 'all-MiniLM-L6-v2'
    
    # Construct the save path relative to this script's location
    # This script is in 'backend/core/memory_rag/'
    # Target is 'backend/core/memory_rag/models/all-MiniLM-L6-v2/'
    try:
        script_dir = os.path.dirname(os.path.abspath(__file__))
        save_path = os.path.join(third_party_path, 'memory_rag_models', model_name)
    except NameError:
        # Fallback for environments where __file__ might not be available
        script_dir = os.getcwd()
        save_path = os.path.join(third_party_path, 'memory_rag_models', model_name)


    logger.info("--- RAG模型下载器 ---")
    logger.info(f"模型名称: {model_name}")
    logger.info(f"目标保存路径: {save_path}")

    if os.path.isdir(save_path) and os.listdir(save_path):
        logger.info("模型似乎已经存在于目标路径，跳过下载。")
        logger.info(f"路径: {save_path}")
        logger.info("如果需要重新下载，请先手动删除此文件夹。")
        return

    os.makedirs(save_path, exist_ok=True)
    logger.info(f"[步骤 1/3] 已创建或确认目录存在: {save_path}")
    
    try:
        if use_mirror:
            logger.info("使用镜像站加速下载...")
            os.environ['HF_ENDPOINT'] = 'https://hf-mirror.com'
        else:
            logger.info("使用默认Hugging Face源下载...")
            logger.info("如果下载失败，请尝试使用 --mirror 参数启用镜像站加速")
        
        from sentence_transformers import SentenceTransformer

        logger.info("[步骤 2/3] 正在从Hugging Face Hub下载模型...")
        logger.info("这个过程可能需要一些时间，取决于您的网络连接。请耐心等待。")
        
        # This step requires an internet connection to Hugging Face
        model = SentenceTransformer(model_name)

        logger.info("[步骤 3/3] 模型下载完成，正在保存到本地磁盘...")
        model.save(save_path)

        logger.info("----------------------------------------")
        logger.info("✅ 模型已成功下载并保存！")
        logger.info(f"   位置: {save_path}")
        logger.info("RAG系统现在可以离线运行了。")
        logger.info("----------------------------------------")

    except Exception as e:
        logger.error(f"下载或保存模型时发生严重错误: {e}")
        logger.error("请检查以下几点：")
        logger.error("  1. 您的网络连接是否正常且可以访问Hugging Face。")
        logger.error("  2. 'sentence-transformers' 和 'torch' 库是否已正确安装。")
        sys.exit(1)

if __name__ == "__main__":
    download_embedding_model()