import asyncio
import uvicorn
import os
import sys
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
            #前端应用只能用于windows（wine之类不管），不如加个判断可以跳过
            if sys.platform.startswith('win32'):
                import subprocess
                root_dir = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
                exe_path = os.path.join(root_dir, "frontend", "LingChatWeb.exe")

                if os.path.exists(exe_path):
                    subprocess.Popen([exe_path])
                    logger.info(f"成功启动前端应用: {exe_path}")
                else:
                    logger.error(f"错误: 找不到可执行文件 {exe_path}")
            else:
                logger.info("非windows系统，不打开前端应用")
            
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