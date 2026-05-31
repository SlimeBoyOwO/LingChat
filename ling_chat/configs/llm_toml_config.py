"""LLM TOML 配置解析器

将 LLM TOML 配置文件解析为与 env_config.py 的 parse_env_file()
保持一致的结构化数据，供前端配置界面展示。
"""

import tomllib
from pathlib import Path
from typing import Any, Dict, List, Optional

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.logger import logger

# ============================================================
# 描述注册表 —— 只登记已知键的描述，未知键 description 留空
# ============================================================

SECTION_DESCRIPTIONS: Dict[str, str] = {
    "main": "主对话模型配置 — 配置 LLM 提供商、API 密钥、模型参数等",
    "translator": "翻译模型配置 — 配置日语翻译功能相关的 LLM 参数",
}

KEY_DESCRIPTIONS: Dict[str, Dict[str, str]] = {
    "main": {
        "provider": "LLM 提供商 (webllm / gemini / ollama / lmstudio)",
        "model": "模型名称",
        "api_key": "API 密钥",
        "base_url": "API 访问地址",
        "proxy": "HTTP 代理地址",
        "temperature": "温度，控制模型输出的随机性，0-2.0 之间 [type:number]",
        "top_p": "核采样，控制模型的选词范围，0-1.0 之间 [type:number]",
        "max_tokens": "最大输出令牌数，控制单次生成的最大长度 [type:number]",
        "enable_thinking": "是否启用模型思考能力，可选值: none / false / true [type:text]",
    },
    "translator": {
        "provider": "翻译模型提供商 (none / webllm / gemini / ollama / lmstudio / qwen-translate)",
        "model": "翻译模型名称",
        "api_key": "翻译模型 API 密钥",
        "base_url": "翻译模型 API 访问地址",
    },
}

PROVIDER_KEY_DESCRIPTIONS: Dict[str, str] = {
    "api_key": "API 密钥",
    "base_url": "API 访问地址",
    "model": "模型名称",
    "proxy": "HTTP 代理地址",
}


# ============================================================
# 辅助函数
# ============================================================

def _get_type_from_description(desc: str) -> str:
    """从描述中提取 [type:xxx] 标注，返回 (清理后的描述, 类型)"""
    import re
    type_match = re.search(r"\[type:\s*(\w+)\s*\]", desc)
    if type_match:
        return type_match.group(1).lower()
    return "text"


def _clean_description(desc: str) -> str:
    """移除描述中的 [type:xxx] 标注，返回纯文本描述"""
    import re
    return re.sub(r"\s*\[type:\s*\w+\s*\]", "", desc).strip()


def _infer_type_from_value(val: Any) -> str:
    """从 Python 值推断类型"""
    if isinstance(val, bool):
        return "bool"
    if isinstance(val, int):
        return "number"
    if isinstance(val, float):
        return "number"
    return "text"


def _format_setting(key: str, value: Any, description: str) -> Dict[str, Any]:
    """构造与 env_config.py 对齐的 setting 条目"""
    raw_type = _get_type_from_description(description)
    inferred = _infer_type_from_value(value)

    entry: Dict[str, Any] = {
        "key": key,
        "value": value,
        "description": _clean_description(description),
        "type": raw_type if raw_type != "text" else inferred,
    }
    return entry


def _get_settings_for_section(
    section_key: str,
    config_dict: Dict[str, Any],
) -> tuple[List[Dict[str, Any]], List[str]]:
    """遍历 section 所有键，查描述注册表，返回 (settings, missing_keys)"""
    settings: List[Dict[str, Any]] = []
    missing: List[str] = []

    section_data = config_dict.get(section_key, {})
    if not isinstance(section_data, dict):
        return settings, missing

    key_descriptions = KEY_DESCRIPTIONS.get(section_key, {})

    for k, v in section_data.items():
        if k in key_descriptions:
            desc = key_descriptions[k]
        elif section_key.startswith("providers") and k in PROVIDER_KEY_DESCRIPTIONS:
            desc = PROVIDER_KEY_DESCRIPTIONS[k]
        else:
            desc = ""
            missing.append(f"{section_key}.{k}")

        settings.append(_format_setting(k, v, desc))

    return settings, missing


# ============================================================
# 核心解析函数
# ============================================================

def parse_llm_toml(config_name: Optional[str] = None) -> Dict[str, Any]:
    """解析单个 LLM TOML 配置，返回与 parse_env_file() 对齐的结构

    Args:
        config_name: 配置方案名称（不含 .toml）。为 None 时读取当前激活配置

    Returns:
        形如 {"LLM 配置方案: <显示名>": {"subcategories": {...}, "missing_keys": [...]}}
    """
    # 1. 读取配置
    if config_name is None:
        config = llm_config.get_active_config()
        display_name = llm_config.get_active_config_name()
    else:
        config_path = llm_config._config_dir / f"{config_name}.toml"
        if not config_path.exists():
            logger.error(f"配置方案不存在: {config_name}")
            return {}
        config = _parse_toml_file(config_path)
        display_name = config_name

    if not config:
        return {}

    # 2. 构造类别名
    scheme_name = config.get("config_name", display_name)
    category_key = f"LLM 配置方案: {scheme_name}"

    # 3. 遍历 sections（排除元数据字段）
    skip_keys = {"config_name", "config_description"}
    subcategories: Dict[str, Any] = {}
    all_missing: List[str] = []

    for section_key, section_value in config.items():
        if section_key in skip_keys:
            continue
        if not isinstance(section_value, dict):
            continue

        # 获取 section 描述
        section_desc = SECTION_DESCRIPTIONS.get(section_key, "")
        if not section_desc:
            logger.debug(f"未知 section: {section_key}")

        settings, missing = _get_settings_for_section(section_key, config)
        all_missing.extend(missing)

        subcategories[section_key] = {
            "description": section_desc,
            "settings": settings,
        }

    result: Dict[str, Any] = {
        category_key: {
            "config_description": config.get("config_description", ""),
            "subcategories": subcategories,
            "missing_keys": all_missing,
        }
    }

    return result


def _parse_toml_file(path: Path) -> Dict[str, Any]:
    """从文件解析 TOML 配置"""
    try:
        with open(path, "rb") as f:
            return tomllib.load(f)
    except Exception as e:
        logger.error(f"解析 TOML 文件失败 {path}: {e}")
        return {}


def get_all_llm_toml_configs() -> Dict[str, Any]:
    """遍历所有 .toml 配置方案，汇总返回"""
    result: Dict[str, Any] = {}
    config_dir: Path = llm_config._config_dir

    for f in sorted(config_dir.glob("*.toml")):
        try:
            parsed = parse_llm_toml(f.stem)
            result.update(parsed)
        except Exception as e:
            logger.warning(f"跳过损坏的配置文件 {f}: {e}")

    return result


def find_key_in_toml(
    key: str,
    config_name: Optional[str] = None,
) -> Optional[Dict[str, Any]]:
    """在所有 TOML 配置中查找某个键

    Args:
        key: 要查找的键名
        config_name: 限定配置方案，为 None 时搜索所有

    Returns:
        包含 config_name 的 setting 条目，未找到返回 None
    """
    config_dir: Path = llm_config._config_dir
    toml_files: List[Path] = []

    if config_name is not None:
        path = config_dir / f"{config_name}.toml"
        if path.exists():
            toml_files.append(path)
    else:
        toml_files = sorted(config_dir.glob("*.toml"))

    for f in toml_files:
        config = _parse_toml_file(f)
        if not config:
            continue

        for section_key, section_value in config.items():
            if not isinstance(section_value, dict):
                continue
            if key in section_value:
                # 查描述
                key_descriptions = KEY_DESCRIPTIONS.get(section_key, {})
                if key in key_descriptions:
                    desc = key_descriptions[key]
                elif section_key.startswith("providers") and key in PROVIDER_KEY_DESCRIPTIONS:
                    desc = PROVIDER_KEY_DESCRIPTIONS[key]
                else:
                    desc = ""

                entry = _format_setting(key, section_value[key], desc)
                entry["config_name"] = f.stem
                return entry

    return None
