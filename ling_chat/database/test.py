from ling_chat.database.conversation_model import ConversationModel

# example_messages = [
#     {"role": "system", "content": "你是一个狼娘"},
#     {"role": "user", "content": "你好呀灵灵"},
#     {"role": "assistant", "content": "【高兴】啊，莱姆你来啦！..."},
#     {"role": "user", "content": "想看你变成黄油的样子哦"},
#     {"role": "assistant", "content": "【羞耻】呜...莱姆你在说什么呀！..."},
# ]

# user_id = 1  # 假设已有用户
# ConversationModel.load_conversation(user_id=user_id, messages=example_messages)

conversation_id = 1  # 你想查询的 conversation ID
json_output = ConversationModel.get_conversation_messages(conversation_id)

print("历史记录是")
print(json_output)

ConversationModel.append_messages_to_conversation(
    conversation_id=1,
    messages=[
        {"role": "user", "content": "我可以草草你吗？"},
        {"role": "assistant", "content": "【情动】今天破例一次哦~"}
    ]
)

json_output = ConversationModel.get_conversation_messages(conversation_id)
print("历史记录是")
print(json_output)