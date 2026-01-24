from datetime import datetime

from fastapi import APIRouter, HTTPException, Request

from ling_chat.core.service_manager import service_manager

from ling_chat.game_database.managers.save_manager import SaveManager
from ling_chat.game_database.managers.user_manager import UserManager
from ling_chat.game_database.managers.role_manager import RoleManager

from ling_chat.utils.function import Function

router = APIRouter(prefix="/api/v1/chat/history", tags=["Chat History"])

@router.get("/list")
async def list_user_conversations(user_id: int, page: int = 1, page_size: int = 10):
    try:
        result = SaveManager.get_user_saves(user_id, page, page_size)
        return {
            "code": 200,
            "data": result
        }
    except Exception as e:
        return {
            "code": 500,
            "msg": "Failed to fetch user conversations",
            "error": str(e)
        }

@router.get("/load")
async def load_user_conversations(user_id: int, conversation_id: int):
    try:
        # result = ConversationModel.get_conversation_messages(conversation_id=conversation_id)
        # character_id = ConversationModel.get_conversation_character(conversation_id=conversation_id)
        # TODO： 后面升级的加载存档，应该如果有运行剧本则加载剧本变量，实现场景重现，当然save的status也是要加载的
        
        line_list = SaveManager.get_gameline_list(save_id=conversation_id)
        role_id = SaveManager.get_chat_main_character_id(save_id=conversation_id)
        
        if(line_list != None):
            if not role_id: return HTTPException(status_code=500, detail="本存档主角色不存在")

            # UserModel.update_user_character(user_id=user_id, character_id=character_id)
            UserManager.update_last_character(user_id=user_id, role_id=role_id)
            UserManager.update_last_save(user_id=user_id, save_id=conversation_id)

            # 1. 验证角色是否存在
            character = RoleManager.get_role_by_id(role_id)
            if not character:
                raise HTTPException(status_code=404, detail="角色不存在")

            # 2. 切换AI服务角色
            character_settings = RoleManager.get_role_settings_by_id(role_id)
            if character_settings is None: return HTTPException(status_code=500, detail="角色不存在")

            character_settings["character_id"] = role_id
            if service_manager.ai_service is not None:
                service_manager.ai_service.import_settings(settings=character_settings)
                service_manager.ai_service.load_lines(line_list, role_id)

            print("成功调用记忆存储")
            return {
                "code": 200,
                "data": "success"
            }
        else:
            return {
                "code": 500,
                "msg": "Failed to load user conversations",
                "error": "加载的数据是空的"
            }

    except Exception as e:
        print("创建conversation的时候出错")
        print(str(e))
        return {
            "code": 500,
            "msg": "Failed to load user conversations",
            "error": str(e)
        }

@router.post("/create")  # 改为POST方法
async def create_user_conversations(request: Request):
    try:
        # 从请求体获取JSON数据
        payload = await request.json()
        user_id = payload.get("user_id")
        title = payload.get("title")

        # 参数验证
        if not user_id or not title:
            raise HTTPException(status_code=400, detail="缺少必要参数")
        
        if not service_manager.ai_service:
            raise HTTPException(status_code=500, detail="AI服务未初始化")

        # 获取台词列表
        line_list = service_manager.ai_service.get_lines()
        save_id = SaveManager.create_save(user_id=user_id, title=title).id
        if save_id:
            SaveManager.sync_lines(save_id,line_list)
            SaveManager.update_save_main_role(save_id, service_manager.ai_service.character_id)

        # 获取消息记忆
        # messages = service_manager.ai_service.get_memory()
        # if not messages:  # 处理空消息情况
        #     print("消息记录是空的，请检查错误！！！！！！！！")

        # 获取这个对话的角色
        # character_id = service_manager.ai_service.character_id

        # 创建对话
        # conversation_id = ConversationModel.create_conversation(
        #     user_id=user_id,
        #     messages=messages,
        #     character_id=character_id,
        #     title=title
        # )

        return {
            "code": 200,
            "data": {
                "conversation_id": save_id,
                "message": "存档创建成功"
            }
        }

    except HTTPException as he:
        raise he  # 重新抛出已处理的HTTP异常
    except Exception as e:
        print("创建save的时候出错")
        print(str(e))
        raise HTTPException(
            status_code=500,
            detail=f"创建存档失败: {str(e)}"
        )

@router.post("/save")
async def save_user_conversation(request: Request):
    """
    保存/更新现有对话
    请求体格式:
    {
        "user_id": int,
        "conversation_id": int,
        "title": str (可选)
    }
    """
    try:
        # 从请求体获取JSON数据
        payload = await request.json()
        user_id = payload.get("user_id")
        conversation_id = payload.get("conversation_id")
        title = payload.get("title")

        # 参数验证
        if not user_id or not conversation_id:
            raise HTTPException(status_code=400, detail="缺少必要参数(user_id或conversation_id)")
        
        if not service_manager.ai_service:
            raise HTTPException(status_code=500, detail="AI服务未初始化")
        
        # 获取当前消息记忆
        line_list = service_manager.ai_service.get_lines()
        SaveManager.sync_lines(save_id=conversation_id, input_lines=line_list)

        # messages = service_manager.ai_service.get_memory()
        # if not messages:
        #     print("警告: 消息记录是空的，将清空对话内容")

        # 更新对话
        # ConversationModel.change_conversation_messages(
        #     conversation_id=conversation_id,
        #     messages=messages
        # )

        # 如果需要更新标题
        if title:
            SaveManager.update_save_title(save_id=conversation_id, title=title)
            # ConversationModel.update_conversation_title(conversation_id, title)

        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话保存成功",
                "message_count": len(line_list)
            }
        }

    except HTTPException as he:
        raise he
    except ValueError as ve:
        raise HTTPException(status_code=404, detail=str(ve))
    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=f"保存对话失败: {str(e)}"
        )

@router.post("/delete")
async def delete_user_conversation(request: Request):
    """
    删除用户对话
    请求体格式:
    {
        "user_id": int,
        "conversation_id": int
    }
    """
    try:
        # 从请求体获取JSON数据
        payload = await request.json()
        user_id = payload.get("user_id")
        conversation_id = payload.get("conversation_id")

        # 参数验证
        if not user_id or not conversation_id:
            raise HTTPException(status_code=400, detail="缺少必要参数(user_id或conversation_id)")

        # 执行删除
        # deleted = ConversationModel.delete_conversation(conversation_id)
        deleted = SaveManager.delete_save(save_id=conversation_id)

        if not deleted:
            raise HTTPException(status_code=404, detail="对话不存在或已被删除")

        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话删除成功"
            }
        }

    except HTTPException as he:
        raise he
    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=f"删除对话失败: {str(e)}"
        )
