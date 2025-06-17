import asyncio
import uvicorn
import os
from dotenv import load_dotenv
from core.logger import logger

load_dotenv()

class Server:
    def __init__(self, app):
        self.app = app
        self.logo = [
            "", 
            "", 
            "█╗       ██╗ ███╗   ██╗  ██████╗      █████╗ ██╗  ██╗  █████╗  ████████╗",
            "██║      ██║ ████╗  ██║ ██╔════╝     ██╔═══╝ ██║  ██║ ██╔══██╗ ╚══██╔══╝",
            "██║      ██║ ██╔██╗ ██║ ██║  ███╗    ██║     ███████║ ███████║    ██║   ",
            "██║      ██║ ██║╚██╗██║ ██║   ██║    ██║     ██╔══██║ ██╔══██║    ██║   ",
            "███████╗ ██║ ██║ ╚████║ ╚██████╔╝     █████╗ ██║  ██║ ██║  ██║    ██║   ",
            "╚══════╝ ╚═╝ ╚═╝  ╚═══╝  ╚═════╝      ╚════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝    ╚═╝   "
        ]

    async def start_frontend_app(self):
        try:
            import subprocess
            root_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
            lingchat_exe = os.path.join(root_dir, "frontend", "LingChatWeb.exe")
            
            if os.path.exists(lingchat_exe):
                subprocess.Popen(lingchat_exe)
            else:
                logger.error(f"错误: 找不到 {lingchat_exe}")
        except Exception as e:
            logger.error(f"启动前端应用失败: {e}")

    async def run(self):
        for line in self.logo:
            print(line)
        print("")

        config = uvicorn.Config(
            self.app,
            host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
            port=int(os.getenv('BACKEND_PORT', '8765')),
            log_level="info"
        )
        server = uvicorn.Server(config)
        
        try:
            print("正在启动HTTP服务器...")
            server_task = asyncio.create_task(server.serve())
            
            while not server.started:
                await asyncio.sleep(0.1)
                    
            await self.start_frontend_app()
            await server_task
            
        except Exception as e:
            logger.error(f"服务器启动错误: {e}")