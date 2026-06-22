"""工具调用相关的类型定义

定义统一的工具调用数据结构，使用 OpenAI 格式作为标准输入，
各 Provider 内部转换为各自 API 格式。
"""

from typing import Any, Dict, List, Optional

from pydantic import BaseModel, Field


class FunctionDefinition(BaseModel):
    """函数定义（OpenAI 格式）"""

    name: str = Field(description="函数名称")
    description: str = Field(description="函数描述")
    parameters: Dict[str, Any] = Field(
        default_factory=dict, description="函数参数 JSON Schema"
    )


class ToolDefinition(BaseModel):
    """工具定义（OpenAI 格式）"""

    type: str = Field(default="function", description="工具类型")
    function: FunctionDefinition = Field(description="函数定义")


class ToolCall(BaseModel):
    """工具调用（标准化输出）"""

    id: str = Field(description="工具调用唯一标识")
    type: str = Field(default="function", description="工具类型")
    name: str = Field(description="函数名称")
    arguments: Dict[str, Any] = Field(default_factory=dict, description="函数参数值")

    @classmethod
    def from_openai(cls, tool_call: Dict[str, Any]) -> "ToolCall":
        """从 OpenAI 格式解析"""
        function = tool_call.get("function", {})
        arguments_str = function.get("arguments", "{}")
        try:
            import json

            arguments = (
                json.loads(arguments_str)
                if isinstance(arguments_str, str)
                else arguments_str
            )
        except json.JSONDecodeError:
            arguments = {}

        return cls(
            id=tool_call.get("id", ""),
            type=tool_call.get("type", "function"),
            name=function.get("name", ""),
            arguments=arguments,
        )

    @classmethod
    def from_anthropic(cls, block: Dict[str, Any]) -> "ToolCall":
        """从 Anthropic 格式解析"""
        return cls(
            id=block.get("id", ""),
            type="function",
            name=block.get("name", ""),
            arguments=block.get("input", {}),
        )

    @classmethod
    def from_gemini(cls, function_call: Dict[str, Any]) -> "ToolCall":
        """从 Gemini 格式解析"""
        return cls(
            id=function_call.get("id", ""),
            type="function",
            name=function_call.get("name", ""),
            arguments=function_call.get("args", {}),
        )

    def to_openai(self) -> Dict[str, Any]:
        """转换为 OpenAI 格式"""
        import json

        return {
            "id": self.id,
            "type": self.type,
            "function": {
                "name": self.name,
                "arguments": json.dumps(self.arguments, ensure_ascii=False),
            },
        }

    def to_anthropic(self) -> Dict[str, Any]:
        """转换为 Anthropic 格式"""
        return {
            "type": "tool_use",
            "id": self.id,
            "name": self.name,
            "input": self.arguments,
        }


class ToolResult(BaseModel):
    """工具执行结果"""

    tool_call_id: str = Field(description="对应的工具调用 ID")
    name: str = Field(description="函数名称")
    content: str = Field(description="执行结果内容")
    is_error: bool = Field(default=False, description="是否执行出错")

    def to_openai_message(self) -> Dict[str, Any]:
        """转换为 OpenAI 格式的 tool 消息"""
        return {
            "role": "tool",
            "tool_call_id": self.tool_call_id,
            "name": self.name,
            "content": self.content,
        }


class LLMResponse(BaseModel):
    """LLM 响应（标准化）"""

    content: Optional[str] = Field(default=None, description="文本内容")
    tool_calls: List[ToolCall] = Field(default_factory=list, description="工具调用列表")
    is_finished: bool = Field(default=True, description="是否已完成")

    @property
    def has_tool_calls(self) -> bool:
        """是否包含工具调用"""
        return len(self.tool_calls) > 0


class ToolConverter:
    """工具定义格式转换器"""

    @staticmethod
    def to_anthropic(tools: List[ToolDefinition]) -> List[Dict[str, Any]]:
        """转换为 Anthropic 格式

        OpenAI: {type, function: {name, description, parameters}}
        Anthropic: {name, description, input_schema}
        """
        result = []
        for tool in tools:
            result.append(
                {
                    "name": tool.function.name,
                    "description": tool.function.description,
                    "input_schema": tool.function.parameters,
                }
            )
        return result

    @staticmethod
    def to_gemini(tools: List[ToolDefinition]) -> List[Dict[str, Any]]:
        """转换为 Gemini 格式

        OpenAI: {type, function: {name, description, parameters}}
        Gemini: {functionDeclarations: [{name, description, parameters}]}
        """
        declarations = []
        for tool in tools:
            declarations.append(
                {
                    "name": tool.function.name,
                    "description": tool.function.description,
                    "parameters": tool.function.parameters,
                }
            )
        return [{"functionDeclarations": declarations}]

    @staticmethod
    def to_openai(tools: List[ToolDefinition]) -> List[Dict[str, Any]]:
        """转换为 OpenAI 格式（透传）"""
        return [tool.model_dump() for tool in tools]
