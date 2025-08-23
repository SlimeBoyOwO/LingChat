import random
import sys

CATEGORIZED_MESSAGES = [
    {
        "name": "加载提示",
        "weight": 0.5,
        "messages": [
            "万恶的Python在导入他忠诚的pytorch...",
            "LingChat 正在努力加载中... 稍等一下哦",
        ],
    },
    {
        "name": "其他彩蛋",
        "weight": 0.3,
        "messages": [
         "给LingChat点点star喵，给LingChat点点star谢谢喵",
         "LingChat不能失去彩蛋，就像西方不能失去耶路撒冷！"
        ],
    },
    {
        "name": "开发者彩蛋",
        "weight": 0.2,
        "messages": [
            "你知道吗？钦灵本人比AI钦灵更可爱",
            "其实风雪并不会写代码，她只是趴在键盘上睡着了，然后恰好对LingChat提交了commit",
            "本项目的Python只用0.1秒就可以导入完全部的依赖，剩下的时间只是它在尝试把自己编译成go以防被vickko删除",
            "正在加载PL的代码... 等等，编译器找不到bug，它以为自己出错了。",
            "你们看到云小姐了吗？嗯嗯，她没有失踪也没有怎么样的，我们只是想让你知道，她很可爱",
            "喵？喵~ 喵！",
            "uwa的代码是用来让你知道，钦灵的代码才是最好的!",
        ],
    },
]

DEFAULT_MESSAGE = "正在努力加载中..."

def get_random_loading_message() -> str:
    """
    根据预设的类别和权重，随机选择一条加载文案。
    这个函数是可扩展的，你只需要修改上面的 CATEGORIZED_MESSAGES 列表即可。
    :return: 随机选择的加载文案字符串。
    """
    valid_categories = [cat for cat in CATEGORIZED_MESSAGES if cat.get("messages")]

    if not valid_categories:
        return DEFAULT_MESSAGE

    weights = [cat["weight"] for cat in valid_categories]

    try:

        chosen_category = random.choices(valid_categories, weights=weights, k=1)[0]

        return random.choice(chosen_category["messages"])

    except (IndexError, KeyError) as e:
        print(f"获取随机文案时出错: {e}", file=sys.stderr)
        return DEFAULT_MESSAGE

if __name__ == '__main__':
    print("--- 测试随机文案生成 ---")
    results = {}
    for _ in range(1000):
        msg = get_random_loading_message()
        for cat in CATEGORIZED_MESSAGES:
            if msg in cat["messages"]:
                cat_name = cat["name"]
                results[cat_name] = results.get(cat_name, 0) + 1
                break

    print("1000次随机选择的类别分布:")
    for name, count in results.items():
        print(f"- {name}: {count} 次")
