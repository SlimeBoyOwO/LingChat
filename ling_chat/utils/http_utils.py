"""HTTP utility functions for LingChat.

基于 httpx 最佳实践的下载工具，支持重定向、超时配置和流式下载。
"""

from pathlib import Path
from typing import Callable, Optional

import httpx
from tqdm import tqdm


def download_file(
    url: str,
    save_path: Path,
    timeout: float = 30.0,
    chunk_size: int = 8192,
    progress_callback: Optional[Callable[[int], None]] = None,
    follow_redirects: bool = True,
    max_redirects: int = 10,
) -> None:
    """使用 httpx 下载文件，支持重定向和进度回调。

    Args:
        url: 下载地址
        save_path: 保存路径
        timeout: 请求超时时间（秒）
        chunk_size: 分块下载大小（字节）
        progress_callback: 进度回调函数，接收 0-100 的整数进度
        follow_redirects: 是否跟随重定向
        max_redirects: 最大重定向次数

    Raises:
        RuntimeError: 下载失败或保存失败时抛出
    """
    try:
        with httpx.Client(
            follow_redirects=follow_redirects,
            max_redirects=max_redirects,
        ) as client:
            with client.stream('GET', url, timeout=timeout) as response:
                response.raise_for_status()

                total_size = int(response.headers.get('content-length', 0))
                downloaded = 0

                with save_path.open('wb') as f:
                    if total_size > 0 and progress_callback is None:
                        # 有文件大小信息，使用 tqdm 显示进度条
                        with tqdm(
                            total=total_size,
                            unit='B',
                            unit_scale=True,
                            desc=f"下载 {save_path.name}",
                            ncols=80
                        ) as pbar:
                            for chunk in response.iter_bytes(chunk_size=chunk_size):
                                if chunk:
                                    f.write(chunk)
                                    downloaded += len(chunk)
                                    pbar.update(len(chunk))
                    elif total_size > 0 and progress_callback is not None:
                        # 使用回调函数报告进度
                        for chunk in response.iter_bytes(chunk_size=chunk_size):
                            if chunk:
                                f.write(chunk)
                                downloaded += len(chunk)
                                progress = int((downloaded / total_size) * 100)
                                progress_callback(progress)
                    elif progress_callback is not None:
                        # 没有文件大小但有回调
                        for chunk in response.iter_bytes(chunk_size=chunk_size):
                            if chunk:
                                f.write(chunk)
                                downloaded += len(chunk)
                    else:
                        # 没有文件大小也没有回调，简单下载
                        for chunk in response.iter_bytes(chunk_size=chunk_size):
                            if chunk:
                                f.write(chunk)
                                downloaded += len(chunk)

    except httpx.TooManyRedirects as e:
        raise RuntimeError(f"重定向次数过多: {url}") from e
    except httpx.TimeoutException as e:
        raise RuntimeError(f"下载超时: {url}") from e
    except httpx.RequestError as e:
        raise RuntimeError(f"网络请求失败: {url}") from e
    except httpx.HTTPStatusError as e:
        raise RuntimeError(f"HTTP错误 {e.response.status_code}: {url}") from e
    except OSError as e:
        raise RuntimeError(f"保存文件失败: {save_path}") from e


def download_to_memory(
    url: str,
    timeout: float = 30.0,
    follow_redirects: bool = True,
    max_redirects: int = 10,
) -> bytes:
    """下载内容到内存。

    Args:
        url: 下载地址
        timeout: 请求超时时间（秒）
        follow_redirects: 是否跟随重定向
        max_redirects: 最大重定向次数

    Returns:
        下载内容的字节数据

    Raises:
        RuntimeError: 下载失败时抛出
    """
    try:
        with httpx.Client(
            follow_redirects=follow_redirects,
            max_redirects=max_redirects,
        ) as client:
            response = client.get(url, timeout=timeout)
            response.raise_for_status()
            return response.content
    except httpx.TooManyRedirects as e:
        raise RuntimeError(f"重定向次数过多: {url}") from e
    except httpx.TimeoutException as e:
        raise RuntimeError(f"下载超时: {url}") from e
    except httpx.RequestError as e:
        raise RuntimeError(f"网络请求失败: {url}") from e
    except httpx.HTTPStatusError as e:
        raise RuntimeError(f"HTTP错误 {e.response.status_code}: {url}") from e


def fetch_json(
    url: str,
    timeout: float = 10.0,
    follow_redirects: bool = True,
    max_redirects: int = 10,
) -> dict:
    """获取 JSON 数据。

    Args:
        url: 请求地址
        timeout: 请求超时时间（秒）
        follow_redirects: 是否跟随重定向
        max_redirects: 最大重定向次数

    Returns:
        JSON 解析后的字典

    Raises:
        RuntimeError: 请求失败或解析失败时抛出
    """
    try:
        with httpx.Client(
            follow_redirects=follow_redirects,
            max_redirects=max_redirects,
        ) as client:
            response = client.get(url, timeout=timeout)
            response.raise_for_status()
            return response.json()
    except httpx.TooManyRedirects as e:
        raise RuntimeError(f"重定向次数过多: {url}") from e
    except httpx.TimeoutException as e:
        raise RuntimeError(f"请求超时: {url}") from e
    except httpx.RequestError as e:
        raise RuntimeError(f"网络请求失败: {url}") from e
    except httpx.HTTPStatusError as e:
        raise RuntimeError(f"HTTP错误 {e.response.status_code}: {url}") from e
    except ValueError as e:
        raise RuntimeError(f"JSON 解析失败: {url}") from e
