from datetime import datetime

from fastapi import APIRouter, HTTPException, Request

from ling_chat.core.service_manager import service_manager
from ling_chat.database.character_model import CharacterModel
from ling_chat.database.conversation_model import ConversationModel
from ling_chat.database.user_model import UserConversationModel, UserModel
from ling_chat.utils.function import Function

router = APIRouter(prefix="/api/v1/chat/history", tags=["Chat History"])

@router.get("/list")
async def list_user_conversations(user_id: int, page: int = 1, page_size: int = 10):
    try:
        result = UserConversationModel.get_user_conversations(user_id, page, page_size)
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
        result = ConversationModel.get_conversation_messages(conversation_id=conversation_id)
        character_id = ConversationModel.get_conversation_character(conversation_id=conversation_id)
        if(result != None):
            UserModel.update_user_character(user_id=user_id, character_id=character_id)
            settings = CharacterModel.get_character_settings_by_id(character_id=character_id)
            settings["character_id"] = character_id
            service_manager.ai_service.import_settings(settings)
            service_manager.ai_service.load_memory(result)
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

        # 获取消息记忆
        messages = service_manager.ai_service.get_memory()
        if not messages:  # 处理空消息情况
            print("消息记录是空的，请检查错误！！！！！！！！")

        # 获取这个对话的角色
        character_id = service_manager.ai_service.character_id

        # 创建对话
        conversation_id = ConversationModel.create_conversation(
            user_id=user_id,
            messages=messages,
            character_id=character_id,
            title=title
        )

        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话创建成功"
            }
        }

    except HTTPException as he:
        raise he  # 重新抛出已处理的HTTP异常
    except Exception as e:
        print("创建conversation的时候出错")
        print(str(e))
        raise HTTPException(
            status_code=500,
            detail=f"创建对话失败: {str(e)}"
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

        # 获取当前消息记忆
        messages = service_manager.ai_service.get_memory()
        if not messages:
            print("警告: 消息记录是空的，将清空对话内容")

        # 更新对话
        ConversationModel.change_conversation_messages(
            conversation_id=conversation_id,
            messages=messages
        )

        # 如果需要更新标题
        if title:
            ConversationModel.update_conversation_title(conversation_id, title)

        return {
            "code": 200,
            "data": {
                "conversation_id": conversation_id,
                "message": "对话保存成功",
                "message_count": len(messages)
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
        deleted = ConversationModel.delete_conversation(conversation_id)

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

@router.post("/process-log")
async def process_log_file(request: Request):
    try:
        data = await request.json()
        content = data.get('content')
        user_id = data.get('user_id')

        if not content or not user_id:
            raise HTTPException(status_code=400, detail="缺少必要参数")

        # 解析日志文件
        dialog_datetime, messages = Function.parse_chat_log(content)
        if not messages:
            raise HTTPException(status_code=400, detail="日志文件未包含有效对话")

        # 添加到记忆系统
        service_manager.ai_service.load_memory(messages)

        # 同时创建新对话记录（可选）
        title = f"导入对话 {datetime.now().strftime('%Y-%m-%d %H:%M')}"
        conversation_id = ConversationModel.create_conversation(
            user_id=user_id,
            messages=messages,
            title=title
        )

        print(f"导入对话为 {dialog_datetime} 的时间成功")
        print(f"导入的信息是 {messages}")


        return {
            "success": True,
            "processed_count": len(messages),
            "conversation_id": conversation_id
        }

    except Exception as e:
        raise HTTPException(status_code=500, detail=f"处理日志文件失败: {str(e)}")
