# editor 项目概览

## 项目结构
```text
editor/
├── backend/
│   └── main.py
├── frontend/
│   ├── public/
│   │   └── vite.svg
│   ├── src/
│   │   ├── assets/
│   │   │   └── react.svg
│   │   ├── components/
│   │   │   └── FormEditor.tsx
│   │   ├── App.css
│   │   ├── App.tsx
│   │   ├── EditorPanel.tsx
│   │   ├── StoryNode.tsx
│   │   ├── index.css
│   │   ├── main.tsx
│   │   └── types.ts
│   ├── .gitignore
│   ├── README.md
│   ├── eslint.config.js
│   ├── index.html
│   ├── package.json
│   ├── postcss.config.js
│   ├── tailwind.config.js
│   ├── tsconfig.app.json
│   ├── tsconfig.json
│   ├── tsconfig.node.json
│   └── vite.config.ts
├── story_pack/
│   └── story/
├── story_project/
│   └── story/
└── main.py
```

## 文件内容
### 文件: `frontend/README.md`

```markdown
# React + TypeScript + Vite

This template provides a minimal setup to get React working in Vite with HMR and some ESLint rules.

Currently, two official plugins are available:

- [@vitejs/plugin-react](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react) uses [Babel](https://babeljs.io/) (or [oxc](https://oxc.rs) when used in [rolldown-vite](https://vite.dev/guide/rolldown)) for Fast Refresh
- [@vitejs/plugin-react-swc](https://github.com/vitejs/vite-plugin-react/blob/main/packages/plugin-react-swc) uses [SWC](https://swc.rs/) for Fast Refresh

## React Compiler

The React Compiler is not enabled on this template because of its impact on dev & build performances. To add it, see [this documentation](https://react.dev/learn/react-compiler/installation).

## Expanding the ESLint configuration

If you are developing a production application, we recommend updating the configuration to enable type-aware lint rules:

```js
export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      // Other configs...

      // Remove tseslint.configs.recommended and replace with this
      tseslint.configs.recommendedTypeChecked,
      // Alternatively, use this for stricter rules
      tseslint.configs.strictTypeChecked,
      // Optionally, add this for stylistic rules
      tseslint.configs.stylisticTypeChecked,

      // Other configs...
    ],
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.node.json', './tsconfig.app.json'],
        tsconfigRootDir: import.meta.dirname,
      },
      // other options...
    },
  },
])
```

You can also install [eslint-plugin-react-x](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-x) and [eslint-plugin-react-dom](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-dom) for React-specific lint rules:

```js
// eslint.config.js
import reactX from 'eslint-plugin-react-x'
import reactDom from 'eslint-plugin-react-dom'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      // Other configs...
      // Enable lint rules for React
      reactX.configs['recommended-typescript'],
      // Enable lint rules for React DOM
      reactDom.configs.recommended,
    ],
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.node.json', './tsconfig.app.json'],
        tsconfigRootDir: import.meta.dirname,
      },
      // other options...
    },
  },
])
```
```

### 文件: `backend/main.py`

```python
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
```

### 文件: `frontend/eslint.config.js`

```javascript
import js from '@eslint/js'
import globals from 'globals'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'
import tseslint from 'typescript-eslint'
import { defineConfig, globalIgnores } from 'eslint/config'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
    ],
    languageOptions: {
      ecmaVersion: 2020,
      globals: globals.browser,
    },
  },
])
```

### 文件: `frontend/index.html`

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>frontend</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

### 文件: `frontend/package.json`

```
{
  "name": "frontend",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "lint": "eslint .",
    "preview": "vite preview",
    "init-tailwind": "node ./node_modules/tailwindcss/cli.js init -p"
  },
  "dependencies": {
    "axios": "^1.13.2",
    "clsx": "^2.1.1",
    "js-yaml": "^4.1.1",
    "lucide-react": "^0.554.0",
    "react": "^19.2.0",
    "react-dom": "^19.2.0",
    "reactflow": "^11.11.4",
    "tailwind-merge": "^3.4.0"
  },
  "devDependencies": {
    "@eslint/js": "^9.39.1",
    "@types/js-yaml": "^4.0.9",
    "@types/node": "^24.10.1",
    "@types/react": "^19.2.5",
    "@types/react-dom": "^19.2.3",
    "@vitejs/plugin-react": "^5.1.1",
    "autoprefixer": "^10.4.22",
    "eslint": "^9.39.1",
    "eslint-plugin-react-hooks": "^7.0.1",
    "eslint-plugin-react-refresh": "^0.4.24",
    "globals": "^16.5.0",
    "postcss": "^8.5.6",
    "tailwindcss": "^3.4.17",
    "typescript": "~5.9.3",
    "typescript-eslint": "^8.46.4",
    "vite": "^7.2.4"
  }
}
```

### 文件: `frontend/postcss.config.js`

```javascript
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
}
```

### 文件: `frontend/src/App.css`

```css
#root {
  max-width: 1280px;
  margin: 0 auto;
  padding: 2rem;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: filter 300ms;
}
.logo:hover {
  filter: drop-shadow(0 0 2em #646cffaa);
}
.logo.react:hover {
  filter: drop-shadow(0 0 2em #61dafbaa);
}

@keyframes logo-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: no-preference) {
  a:nth-of-type(2) .logo {
    animation: logo-spin infinite 20s linear;
  }
}

.card {
  padding: 2em;
}

.read-the-docs {
  color: #888;
}
```

### 文件: `frontend/src/App.tsx`

```
import { useState, useCallback, useEffect } from 'react';
import ReactFlow, { 
  addEdge, 
  Background, 
  Controls, 
  MiniMap,
  useNodesState, 
  useEdgesState,
  MarkerType,
  type Connection,
  type Edge,
  type Node,
  type NodeMouseHandler
} from 'reactflow';
import 'reactflow/dist/style.css';
import axios from 'axios';
import jsyaml from 'js-yaml';
import { PlusCircle, Terminal, Cpu } from 'lucide-react';

import StoryNode from './StoryNode';
import EditorPanel from './EditorPanel';
import type { StoryUnitData } from './types';

const nodeTypes = { storyNode: StoryNode };
const API_URL = 'http://localhost:8000';

export default function App() {
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [editorContent, setEditorContent] = useState('');
  const [isEditorOpen, setIsEditorOpen] = useState(false);

  // --- API ---
  const saveFileToBackend = async (filename: string, content: string) => {
    await axios.post(`${API_URL}/file`, { filename, content });
  };

  const fetchFiles = async () => {
    try {
      const res = await axios.get<string[]>(`${API_URL}/files`);
      const fileList = res.data;
      
      const newNodes: Node[] = [];
      const loadedFiles: Record<string, string> = {};

      // 1. Nodes
      // 简单的网格布局算法
      let x = 0, y = 0;
      const GRID_WIDTH = 400;
      const MAX_PER_ROW = 4;
      
      for (let i = 0; i < fileList.length; i++) {
        const file = fileList[i];
        const contentRes = await axios.get(`${API_URL}/file/${file}`);
        loadedFiles[file] = contentRes.data.content;
        
        // 如果节点已经存在（比如拖拽过），保留位置，否则使用默认网格
        const existingNode = nodes.find(n => n.id === file);
        
        newNodes.push({
          id: file,
          type: 'storyNode',
          position: existingNode ? existingNode.position : { x, y },
          data: { label: file, content: contentRes.data.content },
        });

        // Grid math
        if (!existingNode) {
            x += GRID_WIDTH;
            if ((i + 1) % MAX_PER_ROW === 0) {
                x = 0;
                y += 300;
            }
        }
      }
      
      // 2. Edges
      const newEdges: Edge[] = [];
      newNodes.forEach(node => {
        try {
          const yamlData = jsyaml.load(loadedFiles[node.id]) as StoryUnitData;
          const end = yamlData.EndCondition;
          
          if (end?.Type === 'Linear' && end.NextUnitID) {
            newEdges.push({
              id: `e-${node.id}-${end.NextUnitID}`,
              source: node.id, target: end.NextUnitID, sourceHandle: 'next',
              animated: true, style: { stroke: '#ff9900', strokeWidth: 2 },
              markerEnd: { type: MarkerType.ArrowClosed, color: '#ff9900' },
            });
          } else if (end?.Branches) {
            Object.keys(end.Branches).forEach(branchKey => {
              let target = end.Branches![branchKey];
              if (typeof target === 'object') target = target.NextUnitID;
              
              if (target) {
                newEdges.push({
                  id: `e-${node.id}-${target}-${branchKey}`,
                  source: node.id, target: target, sourceHandle: branchKey,
                  style: { stroke: '#00bcd4', strokeWidth: 2 },
                  markerEnd: { type: MarkerType.ArrowClosed, color: '#00bcd4' },
                });
              }
            });
          }
        } catch (e) {}
      });

      setNodes(newNodes);
      setEdges(newEdges);
    } catch (err) { console.error(err); }
  };

  useEffect(() => { fetchFiles(); }, []);

  // --- Interactions ---

  const onNodeClick: NodeMouseHandler = (_e, node) => {
    setSelectedFile(node.id);
    setEditorContent(node.data.content);
    setIsEditorOpen(true);
  };

  // *** 魔法核心：自动连线并重写 YAML ***
  const onConnect = useCallback(async (params: Connection) => {
    // 1. 视觉上立即连线
    setEdges((eds) => addEdge({ ...params, animated: true, style: { stroke: '#fff' } }, eds));

    const sourceId = params.source;
    const targetId = params.target;
    const handleId = params.sourceHandle; // 'next' 或者是 Branch Key (如 'A', 'B')

    if (!sourceId || !targetId) return;

    // 2. 找到源节点数据
    const sourceNode = nodes.find(n => n.id === sourceId);
    if (!sourceNode) return;

    try {
      const data = jsyaml.load(sourceNode.data.content) as StoryUnitData;
      
      // 3. 智能修改 YAML 对象
      if (!data.EndCondition) data.EndCondition = { Type: 'Linear' };

      if (handleId === 'next' || handleId === null) {
        // 线性连接：强制改为 Linear 并指向目标
        data.EndCondition.Type = 'Linear';
        data.EndCondition.NextUnitID = targetId;
      } else {
        // 分支连接：只修改对应 Key 的目标
        if (!data.EndCondition.Branches) data.EndCondition.Branches = {};
        
        // 检查旧数据是字符串还是对象
        const oldBranchVal = data.EndCondition.Branches[handleId];
        if (typeof oldBranchVal === 'object' && oldBranchVal !== null) {
            data.EndCondition.Branches[handleId] = { ...oldBranchVal, NextUnitID: targetId };
        } else {
            data.EndCondition.Branches[handleId] = targetId;
        }
      }

      // 4. 序列化并保存
      const newYaml = jsyaml.dump(data, { flowLevel: 3 });
      await saveFileToBackend(sourceId, newYaml);

      // 5. 更新本地状态（不用刷新整个页面）
      setNodes(nds => nds.map(n => {
        if (n.id === sourceId) return { ...n, data: { ...n.data, content: newYaml } };
        return n;
      }));
      
      // 如果编辑器开着且正是这个文件，也更新编辑器
      if (isEditorOpen && selectedFile === sourceId) {
          setEditorContent(newYaml);
      }

    } catch (e) {
      alert("连线保存失败：YAML 解析错误");
    }
  }, [nodes, isEditorOpen, selectedFile]);

  const handleSave = async (filename: string, newContent: string) => {
    await saveFileToBackend(filename, newContent);
    // 更新节点数据
    setNodes(nds => nds.map(n => n.id === filename ? { ...n, data: { ...n.data, content: newContent } } : n));
    // 刷新连线（因为 EndCondition 可能变了）
    fetchFiles();
    setIsEditorOpen(false);
  };

  const createNewNode = async () => {
    const name = prompt("请输入新单元文件名 (ID):");
    if (!name) return;
    const tpl = `Events:
  - Type: Narration
    Mode: Preset
    Content: "新的故事开始了..."
EndCondition:
  Type: Linear
  NextUnitID: ""`;
    await handleSave(name, tpl);
  };

  return (
    <div className="w-screen h-screen bg-gemini-bg flex flex-col relative">
      {/* 装饰性背景 */}
      <div className="absolute inset-0 pointer-events-none opacity-20 bg-[linear-gradient(0deg,transparent_24%,rgba(255,153,0,.05)_25%,rgba(255,153,0,.05)_26%,transparent_27%,transparent_74%,rgba(255,153,0,.05)_75%,rgba(255,153,0,.05)_76%,transparent_77%,transparent),linear-gradient(90deg,transparent_24%,rgba(255,153,0,.05)_25%,rgba(255,153,0,.05)_26%,transparent_27%,transparent_74%,rgba(255,153,0,.05)_75%,rgba(255,153,0,.05)_76%,transparent_77%,transparent)] bg-[length:50px_50px]"></div>

      {/* Top Bar */}
      <div className="h-16 border-b border-gemini-border flex items-center px-6 justify-between bg-black/80 backdrop-blur-md z-10 relative shadow-lg">
        <div className="flex items-center gap-4">
          <div className="w-10 h-10 bg-gemini-orange flex items-center justify-center rounded-sm shadow-glow">
            <Terminal size={24} className="text-black" />
          </div>
          <div>
            <h1 className="font-bold tracking-[0.25em] text-xl text-white leading-none flex items-center">
              NEO<span className="text-gemini-orange">CHAT</span> STUDIO
            </h1>
            <div className="flex items-center gap-2 mt-1">
              <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse shadow-[0_0_5px_#0f0]"></span>
              <span className="text-[10px] text-gemini-dim font-bold tracking-widest">SYSTEM ONLINE :: V3.0</span>
            </div>
          </div>
        </div>
        <button 
          onClick={createNewNode}
          className="gemini-btn gemini-btn-primary"
        >
          <PlusCircle size={16} /> NEW UNIT
        </button>
      </div>

      {/* Canvas */}
      <div className="flex-1 relative z-0">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onNodeClick={onNodeClick}
          nodeTypes={nodeTypes}
          fitView
          className="bg-gemini-bg"
        >
          <Background color="#222" gap={25} size={1} />
          <Controls className="!bg-black !border-gemini-border !fill-gemini-orange !rounded-none" />
          <MiniMap 
            nodeColor="#ff9900" 
            maskColor="rgba(5, 5, 5, 0.8)" 
            className="!bg-black !border !border-gemini-border !rounded-none"
          />
        </ReactFlow>

        {/* Editor */}
        {isEditorOpen && selectedFile && (
          <EditorPanel 
            fileName={selectedFile} 
            content={editorContent} 
            onClose={() => setIsEditorOpen(false)}
            onSave={handleSave}
          />
        )}
      </div>
      
      {/* 底部状态栏装饰 */}
      <div className="absolute bottom-4 left-4 z-10 text-[10px] text-gemini-dim flex gap-4 pointer-events-none">
        <span className="flex items-center gap-1"><Cpu size={10}/> MEM: 1024TB OK</span>
        <span className="flex items-center gap-1">SYNC: 100%</span>
      </div>
    </div>
  );
}
```

### 文件: `frontend/src/components/FormEditor.tsx`

```
import React from 'react';
import { Trash2, ArrowUp, ArrowDown, Plus, Layers } from 'lucide-react';
import type { StoryUnitData } from '../types';

interface FormEditorProps {
  data: StoryUnitData;
  onChange: (newData: StoryUnitData) => void;
}

export const FormEditor: React.FC<FormEditorProps> = ({ data, onChange }) => {
  
  // --- Events Helpers ---
  const updateEvent = (index: number, field: string, value: any) => {
    const newEvents = [...(data.Events || [])];
    newEvents[index] = { ...newEvents[index], [field]: value };
    onChange({ ...data, Events: newEvents });
  };

  const addEvent = () => {
    const newEvents = [...(data.Events || []), { Type: 'Narration' as const, Mode: 'Preset' as const, Content: '' }];
    onChange({ ...data, Events: newEvents });
  };

  const removeEvent = (index: number) => {
    const newEvents = [...(data.Events || [])];
    newEvents.splice(index, 1);
    onChange({ ...data, Events: newEvents });
  };

  const moveEvent = (index: number, direction: -1 | 1) => {
    const newEvents = [...(data.Events || [])];
    if (index + direction < 0 || index + direction >= newEvents.length) return;
    [newEvents[index], newEvents[index + direction]] = [newEvents[index + direction], newEvents[index]];
    onChange({ ...data, Events: newEvents });
  };

  // --- EndCondition Helpers ---
  const updateEndType = (type: string) => {
    const newEnd = { ...data.EndCondition, Type: type as any };
    // 重置默认值以防出错
    if (type === 'Linear' && !newEnd.NextUnitID) newEnd.NextUnitID = '';
    if (type !== 'Linear' && !newEnd.Branches) newEnd.Branches = { 'A': '', 'B': '' };
    onChange({ ...data, EndCondition: newEnd });
  };

  const updateBranch = (key: string, targetId: string) => {
     const newBranches = { ...(data.EndCondition.Branches || {}) };
     // 简单处理：如果原来是对象，保留对象结构只改ID，如果是字符串直接改
     const original = newBranches[key];
     if (typeof original === 'object' && original !== null) {
        newBranches[key] = { ...original, NextUnitID: targetId };
     } else {
        newBranches[key] = targetId;
     }
     onChange({ ...data, EndCondition: { ...data.EndCondition, Branches: newBranches } });
  };

  const addBranch = () => {
    const newKey = prompt("输入新分支 Key (例如: OPTION_C):", "C");
    if (newKey) updateBranch(newKey, "");
  }

  return (
    <div className="space-y-8 pb-10">
      
      {/* --- 1. 剧情事件列表 --- */}
      <div className="space-y-4">
        <div className="flex items-center justify-between border-b border-gemini-border pb-2">
          <h3 className="text-gemini-orange font-bold text-xs tracking-[0.2em] flex items-center gap-2">
            <Layers size={12} /> STORY EVENTS
          </h3>
          <button onClick={addEvent} className="gemini-btn gemini-btn-primary py-1 px-2 text-[10px]">
            <Plus size={12} /> ADD EVENT
          </button>
        </div>

        <div className="space-y-3">
          {(!data.Events || data.Events.length === 0) && (
             <div className="text-center py-8 text-gemini-dim text-xs italic border border-dashed border-gemini-border">
               暂无事件，点击上方添加...
             </div>
          )}

          {data.Events?.map((ev, idx) => (
            <div key={idx} className="bg-gemini-panel border border-gemini-border p-3 rounded hover:border-gemini-orange/50 transition-all relative group">
              {/* 操作栏 */}
              <div className="absolute right-2 top-2 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity bg-black/50 backdrop-blur rounded p-1">
                <button onClick={() => moveEvent(idx, -1)} className="p-1 text-gemini-dim hover:text-white"><ArrowUp size={12}/></button>
                <button onClick={() => moveEvent(idx, 1)} className="p-1 text-gemini-dim hover:text-white"><ArrowDown size={12}/></button>
                <button onClick={() => removeEvent(idx)} className="p-1 text-gemini-dim hover:text-red-500"><Trash2 size={12}/></button>
              </div>

              <div className="grid grid-cols-12 gap-2 mb-2">
                <div className="col-span-5">
                  <label className="gemini-label">TYPE</label>
                  <select value={ev.Type} onChange={(e) => updateEvent(idx, 'Type', e.target.value)} className="gemini-select">
                    <option value="Narration">Narration (旁白)</option>
                    <option value="Dialogue">Dialogue (对话)</option>
                    <option value="Player">Player (玩家行动)</option>
                    <option value="Action">Action (系统动作)</option>
                    <option value="SystemAction">SystemAction (LLM后台)</option>
                  </select>
                </div>
                <div className="col-span-4">
                  <label className="gemini-label">MODE</label>
                  <select value={ev.Mode || 'Preset'} onChange={(e) => updateEvent(idx, 'Mode', e.target.value)} className="gemini-select">
                    <option value="Preset">Preset (固定)</option>
                    <option value="Prompt">Prompt (生成)</option>
                    <option value="Input">Input (输入)</option>
                  </select>
                </div>
                {ev.Type === 'Dialogue' && (
                  <div className="col-span-3">
                    <label className="gemini-label">ROLE</label>
                    <input type="text" value={ev.Character || ''} onChange={(e) => updateEvent(idx, 'Character', e.target.value)} className="gemini-input text-center" placeholder="ID" />
                  </div>
                )}
              </div>

              <div>
                <label className="gemini-label">CONTENT</label>
                <textarea 
                  rows={ev.Mode === 'Prompt' ? 4 : 2}
                  value={ev.Content || ''}
                  onChange={(e) => updateEvent(idx, 'Content', e.target.value)}
                  className="gemini-input resize-none leading-relaxed text-xs"
                  placeholder={ev.Mode === 'Prompt' ? "输入 Prompt 指令..." : "输入文本内容..."}
                />
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* --- 2. 流程控制 --- */}
      <div className="space-y-4 pt-4">
        <div className="border-b border-gemini-border pb-2">
          <h3 className="text-gemini-blue font-bold text-xs tracking-[0.2em]">FLOW CONTROL</h3>
        </div>

        <div className="bg-black/30 p-4 border border-gemini-border border-l-4 border-l-gemini-blue">
          <label className="gemini-label">EXIT CONDITION TYPE</label>
          <select 
            value={data.EndCondition?.Type || 'Linear'} 
            onChange={(e) => updateEndType(e.target.value)}
            className="gemini-select mb-4 text-gemini-blue font-bold"
          >
            <option value="Linear">➔ Linear (线性跳转)</option>
            <option value="Branching">⑂ Branching (玩家选项分支)</option>
            <option value="AIChoice">🤖 AI Choice (AI 决策分支)</option>
            <option value="PlayerResponseBranch">💬 Response Branch (语义判断分支)</option>
          </select>

          {/* 线性模式 */}
          {(data.EndCondition?.Type === 'Linear') && (
            <div>
              <label className="gemini-label">NEXT UNIT ID (TARGET)</label>
              <input 
                type="text" 
                disabled
                value={data.EndCondition.NextUnitID || ''} 
                className="gemini-input text-gemini-dim cursor-not-allowed bg-gemini-panel/50"
                placeholder="请在画布上拖拽连线..."
              />
              <p className="text-[10px] text-gemini-orange mt-2 flex items-center gap-1">
                <span className="animate-pulse">●</span> 在画布连线可自动填充此处
              </p>
            </div>
          )}

          {/* 分支模式 */}
          {['Branching', 'AIChoice', 'PlayerResponseBranch'].includes(data.EndCondition?.Type || '') && (
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                 <label className="gemini-label">BRANCHES (OUTLETS)</label>
                 <button onClick={addBranch} className="text-gemini-blue hover:text-white text-[10px] flex items-center gap-1"><Plus size={10}/> ADD KEY</button>
              </div>
              
              {Object.keys(data.EndCondition?.Branches || {}).map((key) => {
                 const val = data.EndCondition!.Branches![key];
                 const target = typeof val === 'object' ? val.NextUnitID : val;
                 
                 return (
                   <div key={key} className="flex items-center gap-2 group">
                     <div className="w-20 text-right font-mono text-xs text-gemini-blue font-bold truncate" title={key}>{key}</div>
                     <div className="text-gemini-dim">→</div>
                     <input 
                       type="text" 
                       readOnly
                       value={target || '未连接'} 
                       className="gemini-input flex-1 text-xs text-gemini-dim"
                     />
                     <button className="text-gemini-dim hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity">
                        <Trash2 size={12} onClick={() => {
                            const newB = {...data.EndCondition.Branches};
                            delete newB[key];
                            onChange({...data, EndCondition: {...data.EndCondition, Branches: newB}});
                        }}/>
                     </button>
                   </div>
                 )
              })}
              <p className="text-[10px] text-gemini-dim mt-1 border-t border-gemini-border/50 pt-1">
                * 添加分支 Key 后保存，画布节点右侧会出现对应颜色的连接点。拖拽该连接点即可设置目标。
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
```

### 文件: `frontend/src/EditorPanel.tsx`

```
import React, { useState, useEffect } from 'react';
import { Save, X, Code, Layout } from 'lucide-react';
import jsyaml from 'js-yaml';
import { FormEditor } from './components/FormEditor';
import type { StoryUnitData } from './types';

interface EditorPanelProps {
  fileName: string;
  content: string;
  onClose: () => void;
  onSave: (name: string, content: string) => void;
}

const EditorPanel: React.FC<EditorPanelProps> = ({ fileName, content, onClose, onSave }) => {
  const [mode, setMode] = useState<'GUI' | 'CODE'>('GUI');
  const [codeContent, setCodeContent] = useState(content);
  const [guiData, setGuiData] = useState<StoryUnitData | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 初始化或文件切换时
  useEffect(() => {
    setCodeContent(content);
    try {
      const parsed = jsyaml.load(content) as StoryUnitData;
      // 确保基本结构存在
      if (!parsed.Events) parsed.Events = [];
      if (!parsed.EndCondition) parsed.EndCondition = { Type: 'Linear', NextUnitID: '' };
      
      setGuiData(parsed);
      setError(null);
      setMode('GUI'); // 默认尝试 GUI
    } catch (e) {
      setError("检测到复杂的 YAML 语法，已自动切换至源码模式。");
      setMode('CODE');
    }
  }, [content]);

  // GUI 变动同步到 Code
  const handleGuiChange = (newData: StoryUnitData) => {
    setGuiData(newData);
    try {
      // flowLevel: 3 保持 YAML 比较简洁，不完全折叠也不完全展开
      const newYaml = jsyaml.dump(newData, { flowLevel: 3, lineWidth: 120 });
      setCodeContent(newYaml);
    } catch (e) { console.error(e); }
  };

  return (
    <div className="fixed right-0 top-0 w-[500px] h-full bg-black/90 backdrop-blur-md border-l border-gemini-border shadow-[0_0_50px_rgba(0,0,0,0.8)] z-50 flex flex-col transition-transform duration-300">
      
      {/* Header */}
      <div className="h-16 flex items-center justify-between px-6 border-b border-gemini-border bg-gemini-panel relative overflow-hidden">
        <div className="absolute top-0 left-0 w-1 h-full bg-gemini-orange"></div>
        <div>
          <div className="text-[10px] text-gemini-dim uppercase tracking-widest mb-1">EDITING UNIT</div>
          <div className="text-gemini-orange font-bold font-mono text-xl tracking-wide truncate w-64">{fileName}</div>
        </div>
        
        <div className="flex items-center gap-2">
          <div className="flex bg-black border border-gemini-border rounded p-1">
            <button 
              onClick={() => setMode('GUI')} 
              disabled={!!error}
              className={`p-1.5 rounded transition-all ${mode === 'GUI' ? 'bg-gemini-orange text-black' : 'text-gemini-dim hover:text-white'}`}
              title="Visual Editor"
            >
              <Layout size={16} />
            </button>
            <button 
              onClick={() => setMode('CODE')} 
              className={`p-1.5 rounded transition-all ${mode === 'CODE' ? 'bg-gemini-orange text-black' : 'text-gemini-dim hover:text-white'}`}
              title="YAML Source"
            >
              <Code size={16} />
            </button>
          </div>
          <button onClick={onClose} className="text-gemini-dim hover:text-white p-2 ml-2"><X size={20} /></button>
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto relative bg-grid-dots">
        {error && mode === 'CODE' && (
            <div className="bg-red-900/20 text-red-400 text-xs p-3 border-b border-red-900 font-mono">
                ! SYSTEM WARNING: {error}
            </div>
        )}

        {mode === 'CODE' ? (
          <textarea
            className="w-full h-full bg-transparent text-green-400 font-mono text-sm p-6 focus:outline-none resize-none leading-relaxed"
            value={codeContent}
            onChange={(e) => setCodeContent(e.target.value)}
            spellCheck={false}
          />
        ) : (
          <div className="p-6">
             {guiData && <FormEditor data={guiData} onChange={handleGuiChange} />}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="p-6 border-t border-gemini-border bg-gemini-panel">
        <button
          onClick={() => onSave(fileName, codeContent)}
          className="gemini-btn gemini-btn-primary w-full py-3 text-sm"
        >
          <Save size={16} />
          SAVE & SYNC
        </button>
      </div>
    </div>
  );
};

export default EditorPanel;
```

### 文件: `frontend/src/index.css`

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  @apply bg-gemini-bg text-gray-300 font-mono overflow-hidden;
}

/* 滚动条美化 */
::-webkit-scrollbar { width: 6px; height: 6px; }
::-webkit-scrollbar-track { @apply bg-gemini-bg; }
::-webkit-scrollbar-thumb { @apply bg-gemini-border rounded; }
::-webkit-scrollbar-thumb:hover { @apply bg-gemini-orange; }

/* 通用 Gemini UI 组件类 */
.gemini-input {
  @apply w-full bg-black border border-gemini-border px-3 py-2 text-sm text-white focus:border-gemini-orange focus:outline-none transition-colors placeholder-gray-700 font-mono;
}

.gemini-select {
  @apply w-full bg-black border border-gemini-border px-3 py-2 text-sm text-white focus:border-gemini-orange focus:outline-none appearance-none cursor-pointer font-mono;
}

.gemini-label {
  @apply block text-[10px] uppercase tracking-widest text-gemini-dim mb-1 font-bold;
}

.gemini-btn {
  @apply px-4 py-1.5 text-xs font-bold uppercase tracking-wider border transition-all flex items-center gap-2 justify-center cursor-pointer;
}

.gemini-btn-primary {
  @apply bg-gemini-orange text-black border-gemini-orange hover:bg-white hover:text-black hover:shadow-glow;
}
```

### 文件: `frontend/src/main.tsx`

```
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
```

### 文件: `frontend/src/StoryNode.tsx`

```
import { memo } from 'react';
import { Handle, Position } from 'reactflow';
import jsyaml from 'js-yaml';
import { FileCode } from 'lucide-react';

interface StoryNodeData {
  label: string;
  content: string;
}

interface StoryNodeProps {
  data: StoryNodeData;
  selected?: boolean;
}

interface YamlData {
  Events?: unknown[];
  EndCondition?: {
    Type?: string;
    NextUnitID?: string;
    Branches?: Record<string, unknown>;
  };
}

const StoryNode = ({ data, selected }: StoryNodeProps) => {
  let parsedData: YamlData = {};
  let endType = 'Linear';
  let branches: string[] = [];

  try {
    parsedData = (jsyaml.load(data.content) as YamlData) || {};
    endType = parsedData.EndCondition?.Type || 'Linear';
    
    if (endType === 'Branching' || endType === 'PlayerResponseBranch' || endType === 'AIChoice') {
      const bData = parsedData.EndCondition?.Branches || {};
      branches = Object.keys(bData);
    }
  } catch (e) {
    console.error("YAML Parse Error", e);
  }

  return (
    <div className={`
      min-w-[200px] bg-gemini-panel border transition-all shadow-lg
      ${selected ? 'border-gemini-orange shadow-[0_0_10px_rgba(255,153,0,0.3)]' : 'border-gemini-border'}
    `}>
      {/* 顶部标题栏 */}
      <div className={`
        px-3 py-2 text-xs font-bold flex items-center gap-2 border-b border-gemini-border
        ${selected ? 'bg-gemini-orange text-black' : 'bg-black text-white'}
      `}>
        <FileCode size={14} />
        {data.label}
      </div>

      {/* 内容预览 */}
      <div className="p-3 text-[10px] text-gemini-dim font-mono bg-gemini-bg/50">
        <div className="flex justify-between items-center mb-2">
          <span className="uppercase">End Condition:</span>
          <span className={`px-1 rounded ${endType !== 'Linear' ? 'text-gemini-blue' : 'text-gemini-orange'}`}>
            {endType}
          </span>
        </div>
        <div className="truncate opacity-50">
          Events: {parsedData.Events?.length || 0}
        </div>
      </div>

      {/* 输入锚点 (左侧) */}
      <Handle type="target" position={Position.Left} className="!bg-gemini-orange !w-3 !h-3 !-left-1.5 rounded-none" />

      {/* 输出锚点 (右侧) - 动态生成 */}
      {endType === 'Linear' || endType === 'Conditional' ? (
        <div className="relative">
            <div className="absolute -right-3 top-[-30px] text-[10px] text-gemini-orange">NEXT</div>
            <Handle type="source" position={Position.Right} id="next" className="!bg-gemini-orange !w-3 !h-3 !-right-1.5 rounded-none" />
        </div>
      ) : (
        <div className="flex flex-col gap-3 py-2 relative">
          {branches.map((branchKey) => (
            <div key={branchKey} className="relative h-4">
              <span className="absolute right-2 text-[10px] text-gemini-blue top-0 uppercase">{branchKey}</span>
              <Handle 
                type="source" 
                position={Position.Right} 
                id={branchKey} 
                style={{ top: '50%' }}
                className="!bg-gemini-blue !w-3 !h-3 !-right-1.5 rounded-none" 
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default memo(StoryNode);
```

### 文件: `frontend/src/types.ts`

```
export interface StoryEvent {
    Type: 'Narration' | 'Dialogue' | 'Player' | 'Action' | 'SystemAction' | 'FreeTime';
    Mode?: 'Preset' | 'Prompt' | 'Input';
    Character?: string; 
    Content?: string;   
    [key: string]: any; // 允许其他字段
  }
  
  export interface EndCondition {
    Type: 'Linear' | 'Branching' | 'AIChoice' | 'PlayerResponseBranch' | 'Conditional';
    NextUnitID?: string;
    Branches?: Record<string, any>; 
  }
  
  export interface StoryUnitData {
    Events: StoryEvent[];
    EndCondition: EndCondition;
  }
```

### 文件: `frontend/tailwind.config.js`

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'gemini-bg': '#050505',      // 极深黑背景
        'gemini-panel': '#0e0e10',   // 面板黑
        'gemini-border': '#2a2a2a',  // 边框灰
        'gemini-orange': '#ff9900',  // 核心高亮橙
        'gemini-dim': '#666666',     // 暗文
        'gemini-blue': '#00bcd4',    // 分支流向色
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', 'monospace'], // 终端字体
      },
      boxShadow: {
        'glow': '0 0 15px rgba(255, 153, 0, 0.2)', // 橙色光晕
      },
      backgroundImage: {
        'grid-dots': 'radial-gradient(#333 1px, transparent 1px)', // 点阵背景
      }
    },
  },
  plugins: [],
}
```

### 文件: `frontend/tsconfig.app.json`

```
{
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.app.tsbuildinfo",
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "types": ["vite/client"],
    "skipLibCheck": true,

    /* Bundler mode */
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",

    /* Linting */
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
  },
  "include": ["src"]
}
```

### 文件: `frontend/tsconfig.json`

```
{
  "files": [],
  "references": [
    { "path": "./tsconfig.app.json" },
    { "path": "./tsconfig.node.json" }
  ]
}
```

### 文件: `frontend/tsconfig.node.json`

```
{
  "compilerOptions": {
    "tsBuildInfoFile": "./node_modules/.tmp/tsconfig.node.tsbuildinfo",
    "target": "ES2023",
    "lib": ["ES2023"],
    "module": "ESNext",
    "types": ["node"],
    "skipLibCheck": true,

    /* Bundler mode */
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "moduleDetection": "force",
    "noEmit": true,

    /* Linting */
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "erasableSyntaxOnly": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
  },
  "include": ["vite.config.ts"]
}
```

### 文件: `frontend/vite.config.ts`

```
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
})
```

### 文件: `main.py`

```python
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
```


请增加三个功能。

- 每个剧情单元的选项卡删除、每个剧情单元选项卡改名功能。改名的话，连着他的上一个选项卡也会自动更新
- 右键连线可以更改颜色和款式，连线颜色款式不再随机
- STORY EVENTS可以直接用鼠标拖动，然后排序而不是必须点上下

以及把表头改成图片的这个样式。文字是，NEO STUDIO PRO，NEO是白色，STUDIO橙色，PRO是灰色小一号的字