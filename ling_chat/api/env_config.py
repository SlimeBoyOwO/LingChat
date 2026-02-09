import re
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional

from fastapi import APIRouter, Body, HTTPException
from pydantic import BaseModel, Field, RootModel

from ling_chat.utils.runtime_config import apply_runtime_config_changes
from ling_chat.utils.runtime_path import package_root

# ==========================================
# Pydantic 模型定义 (数据结构层)
# ==========================================


class SettingType(str, Enum):
    """定义支持的配置项类型"""

    TEXT = "text"
    BOOL = "bool"
    NUMBER = "number"


class SettingItem(BaseModel):
    """单个配置项的数据模型"""

    key: str
    value: Any
    description: str = ""
    type: SettingType = SettingType.TEXT


class SubCategory(BaseModel):
    """子分类模型"""

    description: str = ""
    settings: List[SettingItem] = Field(default_factory=list)


class Category(BaseModel):
    """主分类模型"""

    subcategories: Dict[str, SubCategory] = Field(default_factory=dict)


class ConfigResponse(RootModel[Dict[str, Category]]):
    """API 响应模型"""

    pass


# ==========================================
# 核心逻辑类
# ==========================================


class EnvFileManager:
    """
    专门用于处理自定义格式 .env 文件的管理器。
    支持读取结构化数据和回写更新。
    """

    def __init__(self, file_path: Path):
        self.file_path = file_path

        # 正则表达式预编译
        self.RE_CATEGORY_BEGIN = re.compile(r"^#\s*([^#]+?)\s*BEGIN")
        self.RE_SUBCATEGORY_BEGIN = re.compile(
            r"^##\s*([^#]+?)\s*BEGIN(?:\s*#\s*(.*))?$"
        )
        self.RE_CATEGORY_END = re.compile(r"^#\s*([^#]+?)\s*END")
        self.RE_SUBCATEGORY_END = re.compile(r"^##\s*([^#]+?)\s*END")
        self.RE_ENV_VAR = re.compile(r"^([A-Z_0-9]+)=")
        self.RE_TYPE_HINT = re.compile(r"\[type:\s*(\w+)\s*\]")
        self.RE_UNESCAPED_QUOTE = re.compile(r'(?<!\\)"')

    def parse(self) -> Dict[str, Category]:
        """解析 .env 文件为结构化字典"""
        if not self.file_path.exists():
            return {}

        structured_config: Dict[str, Category] = {}

        # 解析状态上下文
        current_cat_name: Optional[str] = None
        current_sub_name: Optional[str] = None

        # 多行处理状态
        in_multiline = False
        multiline_key: Optional[str] = None
        multiline_buffer: List[str] = []

        with open(self.file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()

        for line in lines:
            line_strip = line.strip()

            # --- 阶段 1: 多行值处理 ---
            if in_multiline:
                multiline_buffer.append(line)
                # 检查是否包含结束引号（简单检查行内是否有非转义引号）
                # 注意：这里假设多行字符串的结束引号在某一行的行尾或注释前
                if '"' in line.split("#")[0]:
                    full_value_raw = "".join(multiline_buffer).strip()
                    self._add_setting(
                        structured_config,
                        current_cat_name,
                        current_sub_name,
                        multiline_key,
                        full_value_raw,
                    )
                    in_multiline = False
                    multiline_buffer = []
                    multiline_key = None
                continue

            # --- 阶段 2: 结构标记处理 ---
            # 跳过空行和无关注释
            if not line_strip or (
                line_strip.startswith("#") and not self._is_structure_tag(line_strip)
            ):
                continue

            if match := self.RE_CATEGORY_BEGIN.match(line_strip):
                current_cat_name = match.group(1).strip()
                if current_cat_name not in structured_config:
                    structured_config[current_cat_name] = Category()
                continue

            if match := self.RE_CATEGORY_END.match(line_strip):
                current_cat_name = None
                continue

            if match := self.RE_SUBCATEGORY_BEGIN.match(line_strip):
                if current_cat_name:
                    current_sub_name = match.group(1).strip()
                    desc = match.group(2).strip() if match.group(2) else ""
                    if (
                        current_sub_name
                        not in structured_config[current_cat_name].subcategories
                    ):
                        structured_config[current_cat_name].subcategories[
                            current_sub_name
                        ] = SubCategory(description=desc)
                continue

            if match := self.RE_SUBCATEGORY_END.match(line_strip):
                current_sub_name = None
                continue

            # --- 阶段 3: 键值对处理 ---
            if match := self.RE_ENV_VAR.match(line):
                if current_cat_name and current_sub_name:
                    key = match.group(1)
                    value_part = line[len(key) + 1 :].strip()

                    unescaped_quotes = len(
                        self.RE_UNESCAPED_QUOTE.findall(value_part.split("#")[0])
                    )

                    # 检查是否为多行起始：以引号开头且引号数量为奇数
                    if value_part.startswith('"') and unescaped_quotes % 2 != 0:
                        in_multiline = True
                        multiline_key = key
                        multiline_buffer = [
                            value_part
                        ]  # 这里不能只是 value_part，因为可能包含换行，但第一行通常是 VALUE="
                    else:
                        self._add_setting(
                            structured_config,
                            current_cat_name,
                            current_sub_name,
                            key,
                            value_part,
                        )

        return structured_config

    def _is_structure_tag(self, line: str) -> bool:
        """辅助判断是否为结构控制行"""
        return bool(
            self.RE_CATEGORY_BEGIN.match(line)
            or self.RE_SUBCATEGORY_BEGIN.match(line)
            or self.RE_CATEGORY_END.match(line)
            or self.RE_SUBCATEGORY_END.match(line)
        )

    def _add_setting(
        self, config: Dict, cat: str, sub: str, key: str, raw_value_block: str
    ):
        """解析单个值块并添加到结构中"""
        # 分离值和注释
        comment_match = re.search(r"\s*#\s*(.*)$", raw_value_block)
        if comment_match:
            full_desc = comment_match.group(1).strip()
            value_str = raw_value_block[: comment_match.start()].strip()
        else:
            full_desc = ""
            value_str = raw_value_block

        # 解析类型标记 如：`[type: bool]`
        setting_type = SettingType.TEXT
        type_match = self.RE_TYPE_HINT.search(full_desc)
        if type_match:
            try:
                setting_type = SettingType(type_match.group(1).lower())
            except ValueError:
                pass  # 未知类型默认为 text
            description = self.RE_TYPE_HINT.sub("", full_desc).strip()
        else:
            description = full_desc

        # 移除包裹的引号
        clean_value = value_str
        if value_str.startswith('"') and value_str.endswith('"'):
            clean_value = value_str[1:-1]

        # 自动推断 bool 类型（为了兼容没有写 [type:bool] 的情况）
        if setting_type == SettingType.TEXT and clean_value.lower() in (
            "true",
            "false",
        ):
            setting_type = SettingType.BOOL

        setting = SettingItem(
            key=key, value=clean_value, description=description, type=setting_type
        )

        config[cat].subcategories[sub].settings.append(setting)

    def save(self, new_values: Dict[str, str]):
        """
        保存配置。
        采用“读取-修改-写入”策略，保留文件中原有的注释和格式。
        """
        with open(self.file_path, "r", encoding="utf-8") as f:
            lines = f.readlines()

        updated_lines = []
        i = 0
        n = len(lines)

        while i < n:
            line = lines[i]
            match = self.RE_ENV_VAR.match(line)

            if not match:
                updated_lines.append(line)
                i += 1
                continue

            key = match.group(1)

            # 识别当前键占据的行数（处理多行值）
            block_lines = [line]
            value_part = line[len(key) + 1 :]

            # 检测多行块
            unescaped_quotes = len(
                self.RE_UNESCAPED_QUOTE.findall(value_part.split("#")[0])
            )
            if value_part.strip().startswith('"') and unescaped_quotes % 2 != 0:
                j = i + 1
                while j < n:
                    block_lines.append(lines[j])
                    if '"' in lines[j].split("#")[0]:
                        break
                    j += 1
                i = j  # 跳过已处理的多行

            # 如果该键在更新列表中
            if key in new_values:
                new_val = str(new_values[key])

                # 尝试保留原有注释
                original_comment = ""
                last_line = block_lines[-1]
                if "#" in last_line:
                    parts = last_line.split("#", 1)
                    if len(parts) > 1:
                        original_comment = " #" + parts[1].rstrip()

                # 格式化新行
                if new_val.lower() in ["true", "false"] or new_val.isdigit():
                    new_line = f"{key}={new_val}{original_comment}\n"
                else:
                    # 包含换行符的字符串，强制使用双引号包裹并换行
                    if "\n" in new_val:
                        new_line = f'{key}="\n{new_val}\n"{original_comment}\n'
                    else:
                        new_line = f'{key}="{new_val}"{original_comment}\n'

                updated_lines.append(new_line)
            else:
                # 键未修改，保留原块
                updated_lines.extend(block_lines)

            i += 1

        with open(self.file_path, "w", encoding="utf-8") as f:
            f.writelines(updated_lines)


# ==========================================
# FastAPI 路由层
# ==========================================

router = APIRouter()
env_file_path = package_root.parent / ".env"
config_manager = EnvFileManager(env_file_path)


@router.get("/api/settings/config", response_model=Dict[str, Category])
async def get_config():
    """获取所有配置项"""
    try:
        return config_manager.parse()
    except Exception as e:
        raise HTTPException(
            status_code=500, detail=f"解析配置文件时发生意外错误: {str(e)}"
        )


@router.post("/api/settings/config")
async def save_config(new_values: Dict[str, Any] = Body(...)):
    """保存并热更新配置"""
    try:
        # 1. 转换值为字符串，确保写入格式正确
        stringified_values = {k: str(v) for k, v in new_values.items()}

        # 2. 保存文件
        config_manager.save(stringified_values)

        # 3. 应用运行时热更新
        apply_runtime_config_changes(stringified_values)

        return {"status": "success", "message": "配置已成功保存并已生效！"}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"保存配置文件失败: {str(e)}")
