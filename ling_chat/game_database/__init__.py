from ling_chat.game_database.database import init_db

# TODO: 包被导入的时候自动初始化数据库，不过这个逻辑建议还是直接在main中实现吧？
init_db()