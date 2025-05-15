from database.user_model import UserModel, UserConversationModel

# 创建用户
# try:
#     user_id = UserModel.create_user("alice", "securepassword123")
#     print(f"创建用户成功，ID: {user_id}")
# except ValueError as ve:
#     print("用户创建失败：", ve)

# 获取用户的所有对话

user_id = 1
conversations = UserConversationModel.get_user_conversations(user_id)
for conv in conversations:
    print(f"对话ID: {conv['id']}, 标题: {conv['title']}, 更新时间: {conv['updated_at']}")