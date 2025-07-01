from mcp.server.fastmcp import FastMCP
from dotenv import load_dotenv
from memory_client import get_memory_client
import logging

load_dotenv()

mcp = FastMCP("graph-rag-memory",version="0.1.0")

def get_memory_client_safe():
    """Get memory client with error handling. Returns None if client cannot be initialized."""
    try:
        return get_memory_client("vector")
    except Exception as e:
        logging.warning(f"Failed to get memory client: {e}")
        return None

@mcp.tool(description="Add a new memory. This method is called everytime the user informs anything about themselves, their preferences, or anything that has any relevant information which can be useful in the future conversation. This can also be called when the user asks you to remember something.")
async def add_memories(text: str) -> str: # TODO: Add character Id type for add memory
    memory_client = get_memory_client_safe()

    if memory_client is None:
        logging.error("Failed to initialize memory client")
        return "Error: Failed to initialize memory client"
    
    try:
        response = memory_client.add(text)
        if response:
            logging.info(f"Memory added successfully: {response}")
            return f"Memory added successfully: {response}"
        else:
            logging.warning("No response from memory client.")
            return "No response from memory client."
    
    except Exception as e:
        logging.exception(f"Error adding to memory: {e}")
        return f"Error adding to memory: {e}"

@mcp.tool()
def search_memory():
    return "search_memory"

@mcp.tool()
def list_memories():
    return "list_memories"

if __name__ == "__main__":
    mcp.run("sse")