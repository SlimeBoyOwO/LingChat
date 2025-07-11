from fastapi import APIRouter, Body, HTTPException, UploadFile

router = APIRouter(prefix="/api/v1/chat/background", tags=["Chat Character"])


@router.get("/audio")
async def index():
    return get_file_response(os.path.join(static_path, "pages", "index.html"))