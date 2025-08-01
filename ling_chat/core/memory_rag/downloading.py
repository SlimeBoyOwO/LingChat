import os
import sys

def download_embedding_model():
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
        save_path = os.path.join(script_dir, 'models', model_name)
    except NameError:
        # Fallback for environments where __file__ might not be available
        script_dir = os.getcwd()
        save_path = os.path.join(script_dir, 'models', model_name)


    print("--- RAG模型下载器 ---")
    print(f"模型名称: {model_name}")
    print(f"目标保存路径: {save_path}")

    if os.path.isdir(save_path) and os.listdir(save_path):
        print("\n[信息] 模型似乎已经存在于目标路径，跳过下载。")
        print(f"路径: {save_path}")
        print("如果需要重新下载，请先手动删除此文件夹。")
        return

    os.makedirs(save_path, exist_ok=True)
    print(f"\n[步骤 1/3] 已创建或确认目录存在: {save_path}")
    
    try:
        mirror_use = str(input("\n[提示] 在中国网络环境中可能无法下载模型，要使用镜像站加速吗？（回答 yes 或者回车忽略）："))
        if mirror_use == "yes":
            os.environ['HF_ENDPOINT'] = 'https://hf-mirror.com'
        
        from sentence_transformers import SentenceTransformer
        
        print("\n[步骤 2/3] 正在从Hugging Face Hub下载模型...")
        print("这个过程可能需要一些时间，取决于您的网络连接。请耐心等待。")
        
        # This step requires an internet connection to Hugging Face
        model = SentenceTransformer(model_name)
        
        print("\n[步骤 3/3] 模型下载完成，正在保存到本地磁盘...")
        model.save(save_path)
        
        print("\n----------------------------------------")
        print("✅ 模型已成功下载并保存！")
        print(f"   位置: {save_path}")
        print("RAG系统现在可以离线运行了。")
        print("----------------------------------------")

    except Exception as e:
        print(f"\n[错误] 下载或保存模型时发生严重错误: {e}", file=sys.stderr)
        print("请检查以下几点：", file=sys.stderr)
        print("  1. 您的网络连接是否正常且可以访问Hugging Face。", file=sys.stderr)
        print("  2. 'sentence-transformers' 和 'torch' 库是否已正确安装。", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    download_embedding_model()