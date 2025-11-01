import zipfile
import py7zr
import requests
import subprocess
import tempfile
from pathlib import Path
from tqdm import tqdm


def download_file(url: str, save_path: Path) -> None:
    try:
        response = requests.get(url, stream=True, timeout=30)
        response.raise_for_status()  # 检查请求是否成功

        # 获取文件总大小
        total_size = int(response.headers.get('content-length', 0))

        with save_path.open('wb') as f:
            if total_size > 0:
                # 有文件大小信息，显示完整进度条
                with tqdm(
                    total=total_size,
                    unit='B',
                    unit_scale=True,
                    desc=f"下载 {save_path.name}",
                    ncols=80
                ) as pbar:
                    for chunk in response.iter_content(chunk_size=8192):
                        if chunk:
                            f.write(chunk)
                            pbar.update(len(chunk))
            else:
                # 没有文件大小信息，只显示下载量
                downloaded_size = 0
                with tqdm(
                    unit='B',
                    unit_scale=True,
                    desc=f"下载 {save_path.name}",
                    ncols=80
                ) as pbar:
                    for chunk in response.iter_content(chunk_size=8192):
                        if chunk:
                            f.write(chunk)
                            downloaded_size += len(chunk)
                            pbar.set_postfix({"已下载": f"{downloaded_size / (1024*1024):.2f} MB"})
                            pbar.update(len(chunk))
    except requests.RequestException as e:
        raise RuntimeError(f"文件下载失败, {url=}") from e
    except OSError as e:
        raise RuntimeError(f"保存文件失败, {save_path=}") from e


def extract_archive(archive_path: Path, extract_to: Path):
    """
    解压压缩文件到指定目录，支持7z和zip格式

    :param archive_path: 压缩文件路径(7z或zip)
    :param extract_to: 解压目标目录
    :raises ValueError: 当文件格式不支持时
    """
    print(f"正在解压 {archive_path} 到 {extract_to}...")

    # 确保目标目录存在
    extract_to.mkdir(parents=True, exist_ok=True)

    # 根据后缀选择解压方式
    suffix = archive_path.suffix.lower()

    if suffix == '.7z':
        with py7zr.SevenZipFile(archive_path, mode='r') as z:
            z.extractall(path=extract_to)
    elif suffix == '.zip':
        with zipfile.ZipFile(archive_path, 'r') as z:
            z.extractall(path=extract_to)
    else:
        raise ValueError(f"不支持的压缩格式: {suffix}. 仅支持 .7z 和 .zip")

    assert extract_to.exists(), f"解压目录 {extract_to} 不存在，请检查路径是否正确。"
    print(f"成功解压 {archive_path} 到 {extract_to}")


def install_from_archive_or_url(dst_path: Path, archive_path: Path | None = None, url: str = ""):
    if dst_path.exists():
        pass
    elif archive_path and archive_path.exists():
        print(f"使用现有的压缩包目录: {archive_path}")
        extract_archive(archive_path, dst_path)
    elif url is not None:
        archive_path = Path(archive_path) if archive_path else Path(tempfile.mktemp())
        print(f"从URL下载并解压到: {archive_path}")
        download_file(url, archive_path)
        print(f"使用压缩包目录: {archive_path}")
        extract_archive(archive_path, dst_path)


def install_vits(vits_path: Path, archive_path: Path | None = None, url: str = ""):
    """
    安装VITS语音合成器
    """
    if archive_path is None:
        default_archive_path = vits_path.parent / "vits-simple-api-windows-cpu-v0.6.16.7z"
        if default_archive_path.exists():
            archive_path = default_archive_path

    url = url or "https://github.com/Artrajz/vits-simple-api/releases/download/v0.6.16/vits-simple-api-windows-cpu-v0.6.16.7z"

    install_from_archive_or_url(vits_path, archive_path, url)


def install_vits_model(vits_path: Path, archive_path: Path | None = None, url: str = ""):
    vits_model_path = vits_path / "data/models/YuzuSoft_Vits"

    if archive_path is None:
        default_archive_path = vits_path.parent / "YuzuSoft_Vits.zip"
        if default_archive_path.exists():
            archive_path = default_archive_path

    url = url or "https://github.com/Zao-chen/zao-chen.github.io/releases/download/%E8%B5%84%E6%BA%90%E4%B8%8B%E8%BD%BD/YuzuSoft_Vits.zip"

    install_from_archive_or_url(vits_model_path, archive_path, url)


def install_sbv2(sbv2_path: Path, archive_path: Path | None = None, url: str = ""):
    """
    安装SBV2语音合成器
    """
    if archive_path is None:
        default_archive_path = sbv2_path.parent / "sbv2.zip"
        if default_archive_path.exists():
            archive_path = default_archive_path

    url = url or "https://github.com/litagin02/Style-Bert-VITS2/releases/download/2.6.0/sbv2.zip"
    install_from_archive_or_url(sbv2_path, archive_path, url)

    install_bat_path = sbv2_path / "Install-Style-Bert-VITS2-CPU.bat"
    subprocess.run([install_bat_path], shell=True, check=True)


def install_18emo(emo_path: Path, url: str = ""):
    """
    安装18emo语音合成器
    """

    url = url or "https://www.modelscope.cn/models/lingchat-research-studio/LingChat-emotion-model-18emo/resolve/master/model.safetensors"
    download_file(url, emo_path / "model.safetensors")

def install_rag_model(use_mirror=False):
    """
    安装RAG系统所需的模型
    """
    # download_embedding_model(use_mirror=use_mirror)
    pass


def main():
    # 示例：安装VITS语音合成器
    vits_path = Path("third_party/vits-simple-api/vits-simple-api-windows-cpu-v0.6.16")
    install_vits(vits_path)

    # 示例：安装VITS模型
    install_vits_model(vits_path)

    # 示例：安装SBV2语音合成器
    sbv2_path = Path("third_party/sbv2/sbv2")
    install_sbv2(sbv2_path)

    # 示例：安装18emo语音合成器
    emo_path = Path("third_party/emotion_model_18emo/")
    install_18emo(emo_path)

    # 安装RAG模型
    install_rag_model()


if __name__ == "__main__":
    main()
