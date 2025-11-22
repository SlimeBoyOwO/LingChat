import os
import glob
import yaml
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel

app = FastAPI()

# 允许前端跨域访问
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# 假设剧本存储在项目根目录的 story 文件夹中
STORY_DIR = "../story_pack/story"
os.makedirs(STORY_DIR, exist_ok=True)

class StoryUnit(BaseModel):
    filename: str
    content: str # YAML string

@app.get("/files")
def list_files():
    """获取所有剧本文件列表"""
    files = glob.glob(os.path.join(STORY_DIR, "*.yaml"))
    # 返回文件名（不含路径）
    return [os.path.basename(f).replace(".yaml", "") for f in files]

@app.get("/file/{filename}")
def get_file(filename: str):
    """读取单个文件内容"""
    path = os.path.join(STORY_DIR, f"{filename}.yaml")
    if not os.path.exists(path):
        # 如果文件不存在，返回默认模板
        default_data = {
            "Events": [{"Type": "Narration", "Mode": "Preset", "Content": "新场景..."}],
            "EndCondition": {"Type": "Linear", "NextUnitID": ""}
        }
        return {"content": yaml.dump(default_data, allow_unicode=True, sort_keys=False)}
    
    with open(path, 'r', encoding='utf-8') as f:
        return {"content": f.read()}

@app.post("/file")
def save_file(unit: StoryUnit):
    """保存文件"""
    path = os.path.join(STORY_DIR, f"{unit.filename}.yaml")
    try:
        # 验证 YAML 格式是否正确
        yaml.safe_load(unit.content)
        with open(path, 'w', encoding='utf-8') as f:
            f.write(unit.content)
        return {"status": "success"}
    except yaml.YAMLError as e:
        raise HTTPException(status_code=400, detail=str(e))

if __name__ == "__main__":
    import uvicorn
    # 运行在 8000 端口
    uvicorn.run(app, host="0.0.0.0", port=8000)