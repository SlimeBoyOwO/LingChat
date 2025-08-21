import unittest
import os
import tempfile
from pathlib import Path
from unittest.mock import patch, mock_open
from ling_chat.utils.function import Function

class TestFunction(unittest.TestCase):
    
    def setUp(self):
        # 创建临时目录用于测试
        self.test_dir = tempfile.TemporaryDirectory()
        self.test_path = Path(self.test_dir.name)
        
    def tearDown(self):
        # 清理临时目录
        self.test_dir.cleanup()
        
    # 测试 detect_language 函数
    def test_detect_language_chinese_abs(self):
        """测试中文文本检测"""
        text = "这是一段中文文本"
        result = Function.detect_language(text)
        self.assertEqual(result, "Chinese_ABS")
        
    def test_detect_language_chinese_mixed(self):
        """测试中日文混合文本检测（中文为主）"""
        text = "这是一段中文文本包含ひらがな"
        result = Function.detect_language(text)
        self.assertEqual(result, "Chinese")
        
    def test_detect_language_japanese(self):
        """测试日文文本检测"""
        text = "これは日本語のテキストです"
        result = Function.detect_language(text)
        self.assertEqual(result, "Japanese")
        
    def test_detect_language_unknown(self):
        """测试未知语言文本检测"""
        text = "12345 !@#$%"
        result = Function.detect_language(text)
        self.assertEqual(result, "Japanese")  # 默认返回Japanese
        
    # 测试 fix_ai_generated_text 函数
    def test_fix_ai_generated_text_no_emotion_tags(self):
        """测试没有情绪标签的文本"""
        text = "这是一段普通文本"
        result = Function.fix_ai_generated_text(text)
        self.assertEqual(result, text)
        
    def test_fix_ai_generated_text_with_emotion_tags(self):
        """测试带情绪标签的文本"""
        text = "【开心】这是一段测试文本<This is test text>（动作描述）"
        result = Function.fix_ai_generated_text(text)
        # 验证括号被替换为全角括号
        self.assertIn("（", result)
        self.assertIn("）", result)
        self.assertNotIn("(", result)
        self.assertNotIn(")", result)
        
    def test_fix_ai_generated_text_multiple_emotion_tags(self):
        """测试多个情绪标签的文本"""
        text = "【开心】这是一段测试文本<This is test text>（动作描述）【悲伤】另一段文本<Another text>"
        result = Function.fix_ai_generated_text(text)
        self.assertIsInstance(result, str)
        
    # 测试 parse_enhanced_txt 函数
    def test_parse_enhanced_txt_single_line_values(self):
        """测试解析单行值"""
        txt_content = """
name = "Test Character"
version = "1.0"
author = 'Test Author'
"""
        txt_file = self.test_path / "settings.txt"
        with open(txt_file, 'w', encoding='utf-8') as f:
            f.write(txt_content)
            
        result = Function.parse_enhanced_txt(txt_file)
        
        self.assertEqual(result['name'], 'Test Character')
        self.assertEqual(result['version'], '1.0')
        self.assertEqual(result['author'], 'Test Author')
        # 检查是否添加了资源路径
        self.assertEqual(result['resource_path'], str(self.test_path))
        
    def test_parse_enhanced_txt_multi_line_values(self):
        """测试解析多行值"""
        txt_content = '''
description = """
这是一段
多行描述文本
"""
name = "Test Character"
'''
        txt_file = self.test_path / "settings.txt"
        with open(txt_file, 'w', encoding='utf-8') as f:
            f.write(txt_content)
            
        result = Function.parse_enhanced_txt(txt_file)
        
        self.assertEqual(result['name'], 'Test Character')
        self.assertIn("这是一段", result['description'])
        self.assertIn("多行描述文本", result['description'])
        self.assertEqual(result['resource_path'], str(self.test_path))
        
    def test_parse_enhanced_txt_hide_none_fields(self):
        """测试隐藏空字段功能"""
        txt_content = """
ai_name = ""
user_name = ""
other_field = ""
"""
        txt_file = self.test_path / "settings.txt"
        with open(txt_file, 'w', encoding='utf-8') as f:
            f.write(txt_content)
            
        result = Function.parse_enhanced_txt(txt_file)
        
        # HIDE_NONE_FIELDS 中的空字段应该为 None
        self.assertIsNone(result['ai_name'])
        self.assertIsNone(result['user_name'])
        # 其他空字段应该为空字符串
        self.assertEqual(result['other_field'], '')
        
    # 测试 parse_chat_log 函数
    def test_parse_chat_log_valid_content(self):
        """测试解析有效的聊天日志"""
        log_content = """对话日期:2023-01-01 12:00:00
用户:你好
钦灵:你好！有什么我可以帮助你的吗？
用户:我想测试聊天记录解析功能
钦灵:好的，我来帮你测试。
"""
        result_datetime, result_records = Function.parse_chat_log(log_content)
        
        self.assertIsNotNone(result_datetime)
        self.assertEqual(len(result_records), 4)
        self.assertEqual(result_records[0]['role'], 'user')
        self.assertEqual(result_records[0]['content'], '你好')
        self.assertEqual(result_records[1]['role'], 'assistant')
        self.assertEqual(result_records[1]['content'], '你好！有什么我可以帮助你的吗？')
        
    def test_parse_chat_log_with_system_setting(self):
        """测试包含系统设定的聊天日志"""
        log_content = """对话日期:2023-01-01 12:00:00
设定:你是一个乐于助人的AI助手
用户:你好
钦灵:你好！
"""
        result_datetime, result_records = Function.parse_chat_log(log_content)
        
        self.assertIsNotNone(result_datetime)
        self.assertEqual(len(result_records), 3)
        self.assertEqual(result_records[0]['role'], 'system')
        self.assertEqual(result_records[0]['content'], '你是一个乐于助人的AI助手')
        
    def test_parse_chat_log_invalid_format(self):
        """测试无效格式的聊天日志"""
        log_content = """无效的格式
用户:你好
钦灵:你好
"""
        result_datetime, result_records = Function.parse_chat_log(log_content)
        
        self.assertIsNone(result_datetime)
        self.assertIsNone(result_records)
        
    def test_parse_chat_log_invalid_datetime(self):
        """测试无效日期时间的聊天日志"""
        log_content = """对话日期:invalid-date
用户:你好
钦灵:你好
"""
        result_datetime, result_records = Function.parse_chat_log(log_content)
        
        self.assertIsNone(result_datetime)
        self.assertIsNone(result_records)
        
    # 测试 load_env 函数
    def test_load_env_simple_vars(self):
        """测试加载简单的环境变量"""
        # 创建一个简单的.env文件
        env_content = """
# 这是一个注释
VAR1=value1
VAR2=value2
# 另一个注释
VAR3 = value3
"""
        env_file = self.test_path / ".env"
        with open(env_file, 'w', encoding='utf-8') as f:
            f.write(env_content)
            
        # 加载环境变量
        result = Function.load_env(env_file)
        
        # 验证结果
        self.assertEqual(result['VAR1'], 'value1')
        self.assertEqual(result['VAR2'], 'value2')
        self.assertEqual(result['VAR3'], 'value3')
        
    def test_load_env_with_quotes(self):
        """测试带引号的环境变量值"""
        env_content = """
VAR1="value with spaces"
VAR2='single quoted value'
VAR3=noquotes
VAR4="value with 'single quotes'"
VAR5='value with "double quotes"'
"""
        env_file = self.test_path / ".env"
        with open(env_file, 'w', encoding='utf-8') as f:
            f.write(env_content)
            
        result = Function.load_env(env_file)
        
        self.assertEqual(result['VAR1'], 'value with spaces')
        self.assertEqual(result['VAR2'], 'single quoted value')
        self.assertEqual(result['VAR3'], 'noquotes')
        self.assertEqual(result['VAR4'], "value with 'single quotes'")
        self.assertEqual(result['VAR5'], 'value with "double quotes"')
        
    def test_load_env_with_comments(self):
        """测试带注释的环境变量"""
        env_content = """
VAR1=value1 # 这是注释
VAR2=value2#这也是注释
VAR3="value3" # 注释
# VAR4=not_exists
VAR5=value5
"""
        env_file = self.test_path / ".env"
        with open(env_file, 'w', encoding='utf-8') as f:
            f.write(env_content)
            
        result = Function.load_env(env_file)
        
        self.assertEqual(result['VAR1'], 'value1')
        self.assertEqual(result['VAR2'], 'value2')
        self.assertEqual(result['VAR3'], 'value3')
        self.assertNotIn('VAR4', result)
        self.assertEqual(result['VAR5'], 'value5')
        
    def test_load_env_multiline_values(self):
        """测试多行值"""
        env_content = '''
VAR1="""这是
一个多行
值"""
VAR2="""单行多值"""
VAR3=normal_value
'''
        env_file = self.test_path / ".env"
        with open(env_file, 'w', encoding='utf-8') as f:
            f.write(env_content)
            
        result = Function.load_env(env_file)
        
        self.assertEqual(result['VAR1'], '这是\n一个多行\n值')
        self.assertEqual(result['VAR2'], '单行多值')
        self.assertEqual(result['VAR3'], 'normal_value')
        
    def test_load_env_empty_and_whitespace(self):
        """测试空行和空白字符"""
        env_content = """
VAR1=value1

# 这是注释

VAR2=value2
   
VAR3=value3
"""
        env_file = self.test_path / ".env"
        with open(env_file, 'w', encoding='utf-8') as f:
            f.write(env_content)
            
        result = Function.load_env(env_file)
        
        self.assertEqual(result['VAR1'], 'value1')
        self.assertEqual(result['VAR2'], 'value2')
        self.assertEqual(result['VAR3'], 'value3')
        self.assertEqual(len(result), 3)  # 只应有3个变量
        
    def test_load_env_file_not_exists(self):
        """测试文件不存在的情况"""
        non_exist_file = self.test_path / "non_exist.env"
        result = Function.load_env(non_exist_file)
        self.assertEqual(result, {})
        
    def test_load_env_init_from_example(self):
        """测试从.env.example初始化"""
        # 创建一个简单的.env.example文件在临时目录中
        example_content = "EXAMPLE_VAR=example_value\nEXAMPLE_VAR2=example_value2"
        example_file = self.test_path / ".env.example"
        with open(example_file, 'w', encoding='utf-8') as f:
            f.write(example_content)
            
        # 确保.env文件不存在
        env_file = self.test_path / ".env"
        
        # 切换当前工作目录到临时目录，以便.load_env能够找到我们创建的.env.example文件
        original_cwd = os.getcwd()
        os.chdir(self.test_path)
        
        try:
            self.assertFalse(env_file.exists())
            
            # 使用init=True加载
            result = Function.load_env(env_file, init=True)
            
            # 验证.env文件已被创建
            self.assertTrue(env_file.exists())
            
            # 验证内容已被复制
            with open(env_file, 'r', encoding='utf-8') as f:
                content = f.read()
            self.assertEqual(content, example_content)
            
            # 验证变量被正确加载
            self.assertEqual(result['EXAMPLE_VAR'], 'example_value')
            self.assertEqual(result['EXAMPLE_VAR2'], 'example_value2')
        finally:
            # 恢复原来的工作目录
            os.chdir(original_cwd)
            
    # 测试 extract_archive 函数
    @patch('ling_chat.utils.function.py7zr')
    @patch('ling_chat.utils.function.zipfile')
    def test_extract_archive_7z(self, mock_zipfile, mock_py7zr):
        """测试解压7z文件"""
        archive_path = self.test_path / "test.7z"
        extract_to = self.test_path / "extracted"
        
        # 创建一个模拟的7z文件
        archive_path.touch()
        
        # 调用函数
        Function.extract_archive(archive_path, extract_to)
        
        # 验证目录被创建
        self.assertTrue(extract_to.exists())
        
        # 验证7z文件被处理
        mock_py7zr.SevenZipFile.assert_called_with(archive_path, mode='r')
        
    @patch('ling_chat.utils.function.py7zr')
    @patch('ling_chat.utils.function.zipfile')
    def test_extract_archive_zip(self, mock_zipfile, mock_py7zr):
        """测试解压zip文件"""
        archive_path = self.test_path / "test.zip"
        extract_to = self.test_path / "extracted"
        
        # 创建一个模拟的zip文件
        archive_path.touch()
        
        # 调用函数
        Function.extract_archive(archive_path, extract_to)
        
        # 验证目录被创建
        self.assertTrue(extract_to.exists())
        
        # 验证zip文件被处理
        mock_zipfile.ZipFile.assert_called_with(archive_path, 'r')
        
    @patch('ling_chat.utils.function.py7zr')
    @patch('ling_chat.utils.function.zipfile')
    def test_extract_archive_unsupported_format(self, mock_zipfile, mock_py7zr):
        """测试不支持的压缩格式"""
        archive_path = self.test_path / "test.rar"
        extract_to = self.test_path / "extracted"
        
        # 创建一个模拟的rar文件
        archive_path.touch()
        
        # 调用函数
        Function.extract_archive(archive_path, extract_to)
        
        # 验证目录被创建
        self.assertTrue(extract_to.exists())
        
        # 验证没有调用任何解压方法
        mock_zipfile.ZipFile.assert_not_called()
        mock_py7zr.SevenZipFile.assert_not_called()

if __name__ == '__main__':
    unittest.main()