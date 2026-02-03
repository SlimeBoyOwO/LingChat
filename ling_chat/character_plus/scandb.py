import requests
from pathlib import Path
import json
from datetime import datetime
from ling_chat.core.logger import logger
import os

BASE_URL =os.getenv("COMMUNITY_URL", "https://192.168.0.116:8000").rstrip('/')
def list_pages():
    try:
        response = requests.get(
            f"{BASE_URL}/api/v1/pages",
            timeout=10
        )
        response.raise_for_status()
        data = response.json()
        return data
    except requests.exceptions.RequestException as e:
        logger.error(f"获取页面列表失败: {e}")
        return []
    except json.JSONDecodeError as e:
        logger.error(f"JSON解析失败: {e}")
        return []
    except Exception as e:
        logger.error(f"未知错误: {e}")
        return []
def get_page(uid: str):
    try:
        response = requests.get(f"{BASE_URL}/api/v1/pages/{uid}")
        if response.status_code == 404:
            logger.warning(f"页面 {uid} 不存在")
            return None
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        logger.error(f"获取页面失败: {e}")
        return None
    except json.JSONDecodeError as e:
        logger.error(f"JSON解析失败: {e}")
        return None

def search_pages(keyword: str, pages: list = None):
    if pages is None:
        pages = list_pages()
    
    if not pages:
        return []
    
    keyword_lower = keyword.lower()
    results = []
    
    for page in pages:
        if not isinstance(page, dict):
            continue
        title = page.get("title", "")
        if isinstance(title, str) and keyword_lower in title.lower():
            results.append(page)
            continue
        uploader = page.get("uploader", "")
        if isinstance(uploader, str) and keyword_lower in uploader.lower():
            results.append(page)
            continue
        uid = page.get("uid", "")
        if isinstance(uid, str) and keyword_lower == uid.lower():
            results.append(page)
            continue
    
    return results



