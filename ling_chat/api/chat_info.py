from typing import Optional

from fastapi import APIRouter, Query
from pydantic import BaseModel, Field

from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.schemas.character_settings import CharacterSettings

router = APIRouter(prefix="/api/v1/chat/info", tags=["Chat Info"])


class WebInitData(BaseModel):
    """初始化返回的数据模型"""

    character_settings: CharacterSettings
    current_interact_role_id: int | None = Field(
        default=None, description="当前互动角色"
    )
    onstage_roles_ids: list[int] = Field(default=[], description="当前在台角色")
    background: str = Field(default="", description="背景")
    background_effect: str = Field(default="", description="背景效果")
    background_music: str = Field(default="", description="背景音乐")


class APIResponse(BaseModel):
    """统一的 API 响应格式"""

    code: int
    msg: str = "success"
    data: Optional[WebInitData] = None
    error: Optional[str] = None


@router.get("/init", response_model=APIResponse)
async def init_web_infos(
    client_id: str = Query(..., description="客户端唯一标识"),
    user_id: int = Query(..., description="用户ID"),
):
    try:
        # 1. 初始化或获取服务
        ai_service = service_manager.ai_service or service_manager.init_ai_service()

        # 2. 注册客户端
        await service_manager.add_client(client_id)

        # 3. 提取成员变量，避免多次链式调用
        settings = ai_service.settings
        game_status = ai_service.game_status

        # 4. 构建返回数据 (Pydantic 会自动处理类型转换和默认值)

        result_data = WebInitData(
            character_settings=settings,
            current_interact_role_id=game_status.current_character.role_id
            if game_status.current_character
            else None,
            onstage_roles_ids=[
                role.role_id
                for role in game_status.onstage_roles
                if role.role_id is not None
            ],
            background=game_status.background,
            background_effect=game_status.background_effect,
            background_music=game_status.background_music,
        )

        return APIResponse(code=200, data=result_data)

    except Exception as e:
        import traceback

        traceback.print_exc()
        logger.error(f"初始化游戏信息 {client_id}, 用户 {user_id} 失败了捏")
        return APIResponse(code=500, msg="Failed to fetch user info", error=str(e))
