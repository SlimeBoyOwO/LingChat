import requests
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import get_user_data_path

def download_file(file_url: str):
    try:
        logger.info(f"正在下载: {file_url}")
        resp = requests.get(file_url, stream=True, timeout=30)
        resp.raise_for_status()
        userpath = get_user_data_path()
        dest_path = userpath / "game_data/character"
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        
        total_size = int(resp.headers.get('content-length', 0))
        downloaded = 0
        
        with open(dest_path, "wb") as f:
            for chunk in resp.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)
                    downloaded += len(chunk)
                    if total_size > 0:
                        progress = downloaded / total_size * 100
                        logger.info(f"\r下载进度: {progress:.1f}%", end="")
        
        logger.info(f"\n文件已保存到: {dest_path}")
        return str(dest_path)
    except Exception as e:
        logger.error(f"\n下载失败: {e}")
        return None