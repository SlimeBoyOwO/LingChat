from mcp.server.fastmcp import FastMCP
from memory_rag.graph import MemoryGraph
from memory_rag.vector import MemoryVector

mcp = FastMCP("graph-rag-memory",version="0.1.0")

@mcp.tool()
def add_memories():
    return "add_memories"

@mcp.tool()
def search_memory():
    return "search_memory"

@mcp.tool()
def list_memories():
    return "list_memories"

if __name__ == "__main__":
    mcp.run("sse")