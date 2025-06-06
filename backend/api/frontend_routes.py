from fastapi import APIRouter
from fastapi.responses import FileResponse
import os

router = APIRouter()

root_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
frontend_dir = os.path.join(root_dir, 'frontend', 'public')

@router.get("/")
async def index():
    return FileResponse(os.path.join(frontend_dir, "pages", "index.html"))

@router.get("/about")
async def about():
    return FileResponse(os.path.join(frontend_dir, "pages", "about.html"))