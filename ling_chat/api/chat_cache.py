import os
from pathlib import Path
from typing import Any

from fastapi import APIRouter, HTTPException
from sqlmodel import Session, select

from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.game_database.database import engine
from ling_chat.game_database.models import Line
from ling_chat.utils.runtime_path import package_root, temp_path

router = APIRouter(prefix="/api/v1/chat/cache", tags=["Chat Cache"])

VOICE_EXTENSIONS = {".wav", ".mp3", ".ogg", ".flac", ".m4a", ".webm", ".weba"}


def _resolve_voice_cache_dir() -> Path:
    raw_path = os.environ.get("TEMP_VOICE_DIR")
    if not raw_path:
        return temp_path / "data" / "voice"

    path = Path(raw_path)
    if path.is_absolute():
        return path

    cwd_path = (Path.cwd() / path).resolve()
    package_parent_path = (package_root.parent / path).resolve()
    if package_parent_path.exists() and not cwd_path.exists():
        return package_parent_path
    return cwd_path


def _iter_voice_cache_files(cache_dir: Path) -> list[Path]:
    if not cache_dir.exists():
        return []
    return [
        path
        for path in cache_dir.iterdir()
        if path.is_file() and path.suffix.lower() in VOICE_EXTENSIONS
    ]


def _get_saved_voice_lines(session: Session) -> list[Line]:
    statement = select(Line).where(
        Line.audio_file.is_not(None),  # type: ignore[attr-defined]
        Line.audio_file != "",
    )
    return list(session.exec(statement).all())


def _build_voice_cache_stats() -> dict[str, Any]:
    cache_dir = _resolve_voice_cache_dir()
    cache_files = _iter_voice_cache_files(cache_dir)
    file_names = {path.name for path in cache_files}
    total_bytes = sum(path.stat().st_size for path in cache_files)

    with Session(engine, expire_on_commit=False) as session:
        saved_lines = _get_saved_voice_lines(session)
        saved_names = {
            Path(line.audio_file).name
            for line in saved_lines
            if line.audio_file and line.audio_file.strip()
        }

    missing_names = saved_names - file_names

    return {
        "cache_dir": str(cache_dir),
        "total_bytes": total_bytes,
        "total_size_mb": round(total_bytes / 1024 / 1024, 2),
        "file_count": len(cache_files),
        "saved_voice_count": len(saved_lines),
        "saved_file_count": len(saved_names),
        "missing_file_count": len(missing_names),
    }


def _clear_runtime_audio_refs() -> int:
    ai_service = service_manager.ai_service
    if not ai_service:
        return 0

    cleared = 0
    for line in ai_service.get_lines():
        if getattr(line, "audio_file", None):
            line.audio_file = None
            cleared += 1
    return cleared


@router.get("/voice")
async def get_voice_cache_stats():
    try:
        return {"code": 200, "data": _build_voice_cache_stats()}
    except Exception as e:
        logger.error(f"获取语音缓存信息失败: {e}")
        raise HTTPException(status_code=500, detail=f"获取语音缓存信息失败: {e}")


@router.delete("/voice")
async def clear_voice_cache():
    try:
        cache_dir = _resolve_voice_cache_dir()
        cache_files = _iter_voice_cache_files(cache_dir)
        deleted_files = 0
        deleted_bytes = 0
        failed_files: list[str] = []

        for path in cache_files:
            try:
                size = path.stat().st_size
                path.unlink()
                deleted_files += 1
                deleted_bytes += size
            except Exception as e:
                failed_files.append(path.name)
                logger.warning(f"删除语音缓存失败 {path}: {e}")

        with Session(engine, expire_on_commit=False) as session:
            saved_lines = _get_saved_voice_lines(session)
            cleared_db_refs = len(saved_lines)
            for line in saved_lines:
                line.audio_file = None
                session.add(line)
            session.commit()

        cleared_runtime_refs = _clear_runtime_audio_refs()
        stats = _build_voice_cache_stats()

        return {
            "code": 200,
            "data": {
                "deleted_files": deleted_files,
                "deleted_bytes": deleted_bytes,
                "deleted_size_mb": round(deleted_bytes / 1024 / 1024, 2),
                "cleared_db_refs": cleared_db_refs,
                "cleared_runtime_refs": cleared_runtime_refs,
                "failed_files": failed_files,
                "stats": stats,
            },
        }
    except Exception as e:
        logger.error(f"清理语音缓存失败: {e}")
        raise HTTPException(status_code=500, detail=f"清理语音缓存失败: {e}")
