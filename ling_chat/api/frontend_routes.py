import os
from pathlib import Path

from fastapi import APIRouter
from fastapi import HTTPException
from fastapi.responses import FileResponse, Response
from fastapi.staticfiles import StaticFiles

from ling_chat.utils.runtime_path import static_path, temp_path

frontend_path = static_path / "frontend"
frontend_audio_path = frontend_path / "audio"

router = APIRouter()


# ✅ 自定义 StaticFiles（禁用缓存并设置正确的MIME类型）
class NoCacheStaticFiles(StaticFiles):
    async def get_response(self, path: str, scope):
        response = await super().get_response(path, scope)
        # 设置正确的MIME类型
        if path.endswith(".js"):
            response.headers["Content-Type"] = "application/javascript"
        elif path.endswith(".css"):
            response.headers["Content-Type"] = "text/css"
        elif path.endswith(".html"):
            response.headers["Content-Type"] = "text/html"
        # 设置缓存控制头
        response.headers["Cache-Control"] = "no-cache, no-store, must-revalidate"
        response.headers["Pragma"] = "no-cache"
        response.headers["Expires"] = "0"
        return response


# ✅ 托管所有静态资源（保持原有路径结构）
# 注意：这里改为返回 StaticFiles 实例，由上层 app.mount() 调用
# 如果目录不存在则返回 None，表示仅 API 模式
def is_frontend_available() -> bool:
    """检查前端目录是否存在"""
    return frontend_path.exists()


def get_static_files():
    if not is_frontend_available():
        return None
    return NoCacheStaticFiles(directory=frontend_path)


def get_audio_files():
    """获取临时音频目录的 StaticFiles（用于 TTS 等动态生成的音频）"""
    audio_path = Path(os.environ.get("TEMP_VOICE_DIR", temp_path / "audio"))
    audio_path.mkdir(exist_ok=True)
    return NoCacheStaticFiles(directory=audio_path, html=False)


def get_frontend_audio_file(filename: str) -> Response:
    """
    获取前端静态音频文件
    优先从前端静态目录读取，如果不存在则返回 404
    """
    audio_file = frontend_audio_path / filename
    if audio_file.exists():
        response = FileResponse(
            audio_file,
            media_type="audio/mpeg" if filename.endswith(".mp3") else "audio/wav",
        )
        response.headers["Cache-Control"] = "public, max-age=86400"
        return response
    return Response(status_code=404, content="Audio file not found")


# ✅ 保持原有HTML路由
def get_file_response(file_path: str) -> FileResponse:
    response = FileResponse(file_path)
    response.headers.update(
        {
            "Cache-Control": "no-cache, no-store, must-revalidate",
            "Pragma": "no-cache",
            "Expires": "0",
        }
    )
    return response


@router.get("/")
async def index():
    return get_file_response(str(frontend_path / "index.html"))


# TODO 这个文件新前端不存在
@router.get("/about")
async def about():
    return get_file_response(str(frontend_path / "pages/about.html"))


@router.get("/@/assets/images/default_bg.webp")
async def default_background_alias():
    candidates = list(frontend_path.glob("assets/background2-*.webp"))
    if not candidates:
        raise HTTPException(status_code=404)
    return get_file_response(str(candidates[0]))
