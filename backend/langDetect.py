class LangDetect:
    def __init__(self):
        # 定义Unicode范围
        self.chinese_ranges = [
            (0x4E00, 0x9FFF),    # 基本汉字
            (0x3400, 0x4DBF),    # 扩展A
            (0x20000, 0x2A6DF),  # 扩展B
            (0x2A700, 0x2B73F),  # 扩展C
            (0x2B740, 0x2B81F),  # 扩展D
            (0x2B820, 0x2CEAF),  # 扩展E
            (0xF900, 0xFAFF),    # 兼容汉字
            (0x3300, 0x33FF),    # 兼容符号
        ]
        
        self.japanese_ranges = [
            (0x3040, 0x309F),    # 平假名
            (0x30A0, 0x30FF),    # 片假名
            (0x31F0, 0x31FF),    # 片假名音标扩展
            (0xFF65, 0xFF9F),    # 半角片假名
        ]

    def detect_language(self, text):
        """
        判断输入文本是中文还是日文
        
        参数:
            text (str): 要检测的文本
            
        返回:
            str: "Chinese", "Japanese" 或 "Unknown"
        """
        
        # 统计字符类型
        chinese_count = 0
        japanese_count = 0
        
        for char in text:
            code = ord(char)
            
            # 检查中文
            for start, end in self.chinese_ranges:
                if start <= code <= end:
                    chinese_count += 1
                    break
                    
            # 检查日文
            for start, end in self.japanese_ranges:
                if start <= code <= end:
                    japanese_count += 1
                    break
        
        # 判断结果
        if chinese_count > 0 and japanese_count == 0:
            return "Chinese_ABS"
        elif japanese_count < chinese_count:
            return "Chinese"
        else:
            return "Japanese"

# 测试示例
if __name__ == "__main__":
    detecter = LangDetect()

    test_cases = [
        ("你好，世界！", "Chinese"),
        ("こんにちは、世界！", "Japanese"),
        ("約束します...今後はどんな命令も...", "Unknown"),
        ("分かった...今後は...『毛選』を...同人誌カバーで包んで読むから...", "Japanese"),  # 混合情况
    ]
    
    for text, expected in test_cases:
        result = detecter.detect_language(text)
        print(f"输入: {text} | 检测结果: {result} | 预期: {expected} | {'✓' if result == expected else '✗'}")

    input("等你宝宝")