import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from ling_chat.api.env_config import parse_env_file, save_env_file

class TestEnvConfig(unittest.TestCase):
    def setUp(self):
        # 创建临时目录和文件用于测试
        self.test_dir = tempfile.TemporaryDirectory()
        self.original_env_file_path = Path(__file__).parent.parent / 'ling_chat' / 'api' / 'env_config.py'
        
        # 临时修改env_file_path
        import ling_chat.api.env_config as env_config_module
        self.original_env_file_path = env_config_module.env_file_path
        self.original_tmp_env_file_path = env_config_module.tmp_env_file_path
        env_config_module.env_file_path = Path(self.test_dir.name) / '.env'
        env_config_module.tmp_env_file_path = Path(self.test_dir.name) / '.env.example'
        
        self.env_file_path = env_config_module.env_file_path

    def tearDown(self):
        # 清理临时目录
        self.test_dir.cleanup()
        
        # 恢复原始路径
        import ling_chat.api.env_config as env_config_module
        env_config_module.env_file_path = self.original_env_file_path
        env_config_module.tmp_env_file_path = self.original_tmp_env_file_path

    def test_parse_empty_file(self):
        """测试解析空文件"""
        # 创建空的.env文件
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            pass
        
        result = parse_env_file()
        self.assertIsInstance(result, dict)
        self.assertEqual(len(result), 0)

    def test_parse_simple_config(self):
        """测试解析简单的配置文件"""
        # 创建简单的.env文件
        env_content = '''# 基础设置 BEGIN
## LLM 模型设置 BEGIN # 配置 LLM 和API相关的密钥和地址
LLM_PROVIDER="webllm" # 在这里选择对话模型
CHAT_API_KEY="sk-114514" # API Key
USE_RAG=false # 是否启用RAG系统 [type:bool]
## LLM 模型设置 END
# 基础设置 END'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        result = parse_env_file()
        
        # 检查基本结构
        self.assertIn('基础设置', result)
        self.assertIn('LLM 模型设置', result['基础设置']['subcategories'])
        
        subcategory = result['基础设置']['subcategories']['LLM 模型设置']
        self.assertEqual(subcategory['description'], '配置 LLM 和API相关的密钥和地址')
        self.assertEqual(len(subcategory['settings']), 3)
        
        # 检查第一个设置项
        setting1 = subcategory['settings'][0]
        self.assertEqual(setting1['key'], 'LLM_PROVIDER')
        self.assertEqual(setting1['value'], 'webllm')
        self.assertEqual(setting1['description'], '在这里选择对话模型')
        self.assertEqual(setting1['type'], 'text')
        
        # 检查第二个设置项
        setting2 = subcategory['settings'][1]
        self.assertEqual(setting2['key'], 'CHAT_API_KEY')
        self.assertEqual(setting2['value'], 'sk-114514')
        self.assertEqual(setting2['description'], 'API Key')
        self.assertEqual(setting2['type'], 'text')
        
        # 检查布尔类型设置项
        setting3 = subcategory['settings'][2]
        self.assertEqual(setting3['key'], 'USE_RAG')
        self.assertEqual(setting3['value'], 'false')
        self.assertEqual(setting3['description'], '是否启用RAG系统')
        self.assertEqual(setting3['type'], 'bool')

    def test_parse_multiline_config(self):
        """测试解析多行值配置"""
        # 创建包含多行值的.env文件
        env_content = '''# 基础设置 BEGIN
## 多行测试 BEGIN
RAG_PROMPT_PREFIX="
--- 以下是根据你的历史记忆检索到的相关对话片段，请参考它们来回答当前问题 ---
" # RAG前缀提示，支持多行
SINGLE_LINE="value" # 单行值测试
## 多行测试 END
# 基础设置 END'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        result = parse_env_file()
        
        self.assertIn('基础设置', result)
        self.assertIn('多行测试', result['基础设置']['subcategories'])
        
        subcategory = result['基础设置']['subcategories']['多行测试']
        self.assertEqual(len(subcategory['settings']), 2)
        
        # 检查多行值设置项
        multiline_setting = subcategory['settings'][0]
        self.assertEqual(multiline_setting['key'], 'RAG_PROMPT_PREFIX')
        self.assertIn('以下是根据你的历史记忆检索到的相关对话片段', multiline_setting['value'])
        self.assertEqual(multiline_setting['description'], 'RAG前缀提示，支持多行')
        self.assertEqual(multiline_setting['type'], 'text')
        
        # 检查单行值设置项
        single_line_setting = subcategory['settings'][1]
        self.assertEqual(single_line_setting['key'], 'SINGLE_LINE')
        self.assertEqual(single_line_setting['value'], 'value')
        self.assertEqual(single_line_setting['description'], '单行值测试')
        self.assertEqual(single_line_setting['type'], 'text')

    def test_parse_without_env_file(self):
        """测试没有.env文件的情况"""
        # 确保.env文件不存在
        if self.env_file_path.exists():
            self.env_file_path.unlink()
            
        result = parse_env_file()
        self.assertIsInstance(result, dict)
        self.assertEqual(len(result), 0)

    def test_parse_with_comments_only(self):
        """测试只包含注释的文件"""
        env_content = '''# 这是注释
# 这也是注释
## 还是注释'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        result = parse_env_file()
        self.assertIsInstance(result, dict)
        self.assertEqual(len(result), 0)

    def test_save_env_file_with_simple_values(self):
        """测试保存简单值到.env文件"""
        # 创建一个初始.env文件
        env_content = '''LLM_PROVIDER="webllm" # 在这里选择对话模型
CHAT_API_KEY="sk-114514" # API Key
USE_RAG=false # 是否启用RAG系统 [type:bool]'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        # 定义要更新的值
        new_values = {
            'LLM_PROVIDER': 'ollama',
            'CHAT_API_KEY': 'new-key',
            'USE_RAG': 'true'
        }
        
        # 调用save_env_file函数
        save_env_file(new_values)
        
        # 重新读取文件内容并验证
        with open(self.env_file_path, 'r', encoding='utf-8') as f:
            updated_content = f.read()
        
        # 检查值是否正确更新
        self.assertIn('LLM_PROVIDER="ollama"', updated_content)
        self.assertIn('CHAT_API_KEY="new-key"', updated_content)
        self.assertIn('USE_RAG=true', updated_content)

    def test_save_env_file_with_multiline_values(self):
        """测试保存多行值到.env文件"""
        # 创建一个包含多行值的初始.env文件
        env_content = '''RAG_PROMPT_PREFIX="
--- 以下是根据你的历史记忆检索到的相关对话片段 ---
" # RAG前缀提示，支持多行
SINGLE_LINE="value" # 单行值测试'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        # 定义要更新的值，包括一个多行值
        new_values = {
            'RAG_PROMPT_PREFIX': '--- 新的提示信息 ---\n包含多行内容',
            'SINGLE_LINE': 'new-value'
        }
        
        # 调用save_env_file函数
        save_env_file(new_values)
        
        # 重新读取文件内容并验证
        with open(self.env_file_path, 'r', encoding='utf-8') as f:
            updated_content = f.read()
        
        # 检查多行值是否正确更新
        self.assertIn('RAG_PROMPT_PREFIX="\n--- 新的提示信息 ---\n包含多行内容\n"', updated_content)
        self.assertIn('SINGLE_LINE="new-value"', updated_content)

    def test_save_env_file_with_boolean_and_numeric_values(self):
        """测试保存布尔值和数值到.env文件"""
        # 创建一个初始.env文件
        env_content = '''ENABLE_RAG=false # 是否启用RAG
MAX_RETRIES=3 # 最大重试次数'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        # 定义要更新的值
        new_values = {
            'ENABLE_RAG': 'true',
            'MAX_RETRIES': '5'
        }
        
        # 调用save_env_file函数
        save_env_file(new_values)
        
        # 重新读取文件内容并验证
        with open(self.env_file_path, 'r', encoding='utf-8') as f:
            updated_content = f.read()
        
        # 检查布尔值和数值是否正确更新（不带引号）
        self.assertIn('ENABLE_RAG=true', updated_content)
        self.assertIn('MAX_RETRIES=5', updated_content)

    def test_save_env_file_preserve_unmodified_values(self):
        """测试保存时保留未修改的值"""
        # 创建一个初始.env文件
        env_content = '''LLM_PROVIDER="webllm" # 在这里选择对话模型
CHAT_API_KEY="sk-114514" # API Key
UNMODIFIED_VALUE="keep-this" # 这个值不应该被修改'''
        
        with open(self.env_file_path, 'w', encoding='utf-8') as f:
            f.write(env_content)
        
        # 只更新部分值
        new_values = {
            'LLM_PROVIDER': 'ollama'
        }
        
        # 调用save_env_file函数
        save_env_file(new_values)
        
        # 重新读取文件内容并验证
        with open(self.env_file_path, 'r', encoding='utf-8') as f:
            updated_content = f.read()
        
        # 检查被修改的值
        self.assertIn('LLM_PROVIDER="ollama"', updated_content)
        # 检查未被修改的值仍然存在
        self.assertIn('CHAT_API_KEY="sk-114514"', updated_content)
        self.assertIn('UNMODIFIED_VALUE="keep-this"', updated_content)

if __name__ == '__main__':
    unittest.main()