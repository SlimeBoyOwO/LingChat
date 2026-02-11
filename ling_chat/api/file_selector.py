from fastapi import APIRouter
import tkinter as tk
from tkinter import filedialog

router = APIRouter()


@router.get("/api/settings/select-file")
async def select_file():
    """
    打开文件选择对话框，返回选中的文件路径
    """
    try:
        root = tk.Tk()
        root.withdraw()  # 隐藏主窗口
        root.attributes('-topmost', True)  # 确保对话框在最前面
        
        file_path = filedialog.askopenfilename(
            title="选择语音合成软件",
            filetypes=[
                ("可执行文件", "*.exe"),
                ("批处理文件", "*.bat"),
                ("Python 脚本", "*.py"),
                ("所有文件", "*.*")
            ]
        )
        
        root.destroy()
        
        return {"path": file_path if file_path else ""}
    except Exception as e:
        return {"path": "", "error": str(e)}
