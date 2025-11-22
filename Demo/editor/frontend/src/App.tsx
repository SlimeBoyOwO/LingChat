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