import os

from fastapi import APIRouter

from ling_chat.api.frontend_routes import get_file_response
from ling_chat.utils.runtime_path import static_path

router = APIRouter(prefix="/api/v1/chat/background", tags=["Chat Character"])


@router.get("/audio")
async def index():
    return get_file_response(os.path.join(static_path, "frontend", "index.html"))
