/**
 * App.tsx
 * 
 * 主应用程序组件
 * 包含：ReactFlow 画布、顶部导航栏、右键菜单逻辑、以及与后端的 API 交互
 */

import { useState, useCallback, useEffect } from 'react';
import ReactFlow, { 
  Background,
  BackgroundVariant, // 引入背景变体以支持网格线
  Controls, 
  MiniMap,
  useNodesState, 
  useEdgesState,
  MarkerType,
  type Connection,
  type Edge,
  type Node,
  type NodeMouseHandler,
  type EdgeMouseHandler
} from 'reactflow';
import 'reactflow/dist/style.css';
import axios from 'axios';
import jsyaml from 'js-yaml';
import { PlusCircle, Cpu } from 'lucide-react';

import StoryNode from './StoryNode';
import EditorPanel from './EditorPanel';
import type { StoryUnitData, VisualConfig } from './types';

// 注册自定义节点类型
const nodeTypes = { storyNode: StoryNode };
const API_URL = 'http://localhost:8000';

/**
 * 自定义右键菜单组件
 */
const ContextMenu = ({ x, y, onClose, options }: { x: number, y: number, onClose: () => void, options: { label: string, action: () => void }[] }) => (
  <div 
    className="fixed bg-black border border-gemini-orange z-50 shadow-[0_0_20px_rgba(255,153,0,0.2)] flex flex-col py-1 min-w-[160px]"
    style={{ top: y, left: x }}
  >
    {options.map((opt, i) => (
      <button 
        key={i} 
        className="text-left px-4 py-2.5 text-xs font-bold text-gray-300 hover:bg-gemini-orange hover:text-black transition-colors tracking-wide font-mono"
        onClick={(e) => { e.stopPropagation(); opt.action(); onClose(); }}
      >
        {opt.label}
      </button>
    ))}
  </div>
);

export default function App() {
  // ReactFlow 状态
  const [nodes, setNodes, onNodesChange] = useNodesState([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState([]);
  
  // 编辑器状态
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [editorContent, setEditorContent] = useState('');
  const [isEditorOpen, setIsEditorOpen] = useState(false);

  // 右键菜单状态
  const [menu, setMenu] = useState<{x: number, y: number, type: 'NODE'|'EDGE', targetId: string, sourceFile?: string, handleId?: string} | null>(null);

  // --- API 交互 ---

  /** 保存文件到后端 */
  const saveFileToBackend = async (filename: string, content: string) => {
    await axios.post(`${API_URL}/file`, { filename, content });
  };

  /** 获取所有文件并构建图谱 */
  const fetchFiles = async () => {
    try {
      const res = await axios.get<string[]>(`${API_URL}/files`);
      const fileList = res.data;
      
      const newNodes: Node[] = [];
      const loadedFiles: Record<string, string> = {};

      // 1. 构建节点 (Nodes)
      let x = 0, y = 0;
      const GRID_WIDTH = 450; // 加大间距适应网格
      const MAX_PER_ROW = 4;
      
      for (let i = 0; i < fileList.length; i++) {
        const file = fileList[i];
        const contentRes = await axios.get(`${API_URL}/file/${file}`);
        loadedFiles[file] = contentRes.data.content;
        
        // 如果节点已存在，保持位置；否则使用网格排列
        const existingNode = nodes.find(n => n.id === file);
        
        newNodes.push({
          id: file,
          type: 'storyNode',
          position: existingNode ? existingNode.position : { x, y },
          data: { label: file, content: contentRes.data.content },
        });

        if (!existingNode) {
            x += GRID_WIDTH;
            if ((i + 1) % MAX_PER_ROW === 0) { x = 0; y += 350; }
        }
      }
      
      // 2. 构建连线 (Edges)
      const newEdges: Edge[] = [];
      newNodes.forEach(node => {
        try {
          const yamlData = jsyaml.load(loadedFiles[node.id]) as StoryUnitData;
          const end = yamlData.EndCondition;
          const visualConfig = end?._Visual || {}; // 读取视觉配置
          
          // 辅助函数：生成带样式的连线
          const createEdge = (target: string, handle: string, defaultColor: string): Edge => {
             const cfg: VisualConfig = visualConfig[handle] || {};
             const color = cfg.Color || defaultColor;
             const styleType = cfg.Style || 'solid'; 
             
             // 根据样式类型设置 strokeDasharray
             let strokeDasharray: string | undefined;
             if (styleType === 'dashed') {
                strokeDasharray = '5,5';
             } else if (styleType === 'dotted') {
                strokeDasharray = '2,2';
             } else {
                strokeDasharray = '0'; // 显式设置为 '0' 以清除虚线样式
             }
             
             return {
                id: `e-${node.id}-${target}-${handle}`,
                source: node.id, target: target, sourceHandle: handle,
                animated: cfg.Animated !== false, // 默认有动画
                style: { 
                    stroke: color, 
                    strokeWidth: 2,
                    strokeDasharray: strokeDasharray
                },
                markerEnd: { type: MarkerType.ArrowClosed, color: color },
                // 将源文件信息存入 data，供右键菜单使用
                data: { sourceFile: node.id, handleId: handle } 
             };
          };

          // 处理 Linear 连接
          if (end?.Type === 'Linear' && end.NextUnitID) {
            newEdges.push(createEdge(end.NextUnitID, 'next', '#ff9900'));
          } 
          // 处理分支连接
          else if (end?.Branches) {
            Object.keys(end.Branches).forEach(branchKey => {
              let target = end.Branches![branchKey];
              if (typeof target === 'object') target = target.NextUnitID;
              if (target) {
                newEdges.push(createEdge(target, branchKey, '#00bcd4'));
              }
            });
          }
        } catch (e) { /* 忽略解析错误 */ }
      });

      setNodes(newNodes);
      setEdges(newEdges);
    } catch (err) { console.error("Fetch failed:", err); }
  };

  useEffect(() => { fetchFiles(); }, []);

  // --- 交互逻辑 ---

  /** 连线事件：自动更新 YAML */
  const onConnect = useCallback(async (params: Connection) => {
    const sourceId = params.source;
    const targetId = params.target;
    const handleId = params.sourceHandle; 
    if (!sourceId || !targetId) return;

    const sourceNode = nodes.find(n => n.id === sourceId);
    if (!sourceNode) return;

    try {
      const data = jsyaml.load(sourceNode.data.content) as StoryUnitData;
      if (!data.EndCondition) data.EndCondition = { Type: 'Linear' };

      // 智能判断连接类型
      if (handleId === 'next' || handleId === null) {
        data.EndCondition.Type = 'Linear';
        data.EndCondition.NextUnitID = targetId;
      } else {
        if (!data.EndCondition.Branches) data.EndCondition.Branches = {};
        const oldBranchVal = data.EndCondition.Branches[handleId];
        // 保留原有对象结构（如果存在）
        if (typeof oldBranchVal === 'object' && oldBranchVal !== null) {
            data.EndCondition.Branches[handleId] = { ...oldBranchVal, NextUnitID: targetId };
        } else {
            data.EndCondition.Branches[handleId] = targetId;
        }
      }
      
      const newYaml = jsyaml.dump(data, { flowLevel: 3 });
      await saveFileToBackend(sourceId, newYaml);
      fetchFiles(); // 刷新以显示连线
    } catch (e) {
      alert("连线失败：YAML 解析错误");
    }
  }, [nodes]);

  // 左键点击节点：打开编辑器
  const onNodeClick: NodeMouseHandler = (_e, node) => {
    setSelectedFile(node.id);
    setEditorContent(node.data.content);
    setIsEditorOpen(true);
  };

  // 右键点击节点：显示菜单
  const onNodeContextMenu: NodeMouseHandler = (e, node) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, type: 'NODE', targetId: node.id });
  };

  // 右键点击连线：显示菜单
  const onEdgeContextMenu: EdgeMouseHandler = (e, edge) => {
    e.preventDefault();
    if(edge.data?.sourceFile && edge.data?.handleId) {
        setMenu({ 
            x: e.clientX, y: e.clientY, 
            type: 'EDGE', 
            targetId: edge.id, 
            sourceFile: edge.data.sourceFile, 
            handleId: edge.data.handleId 
        });
    }
  };

  // --- 功能操作 ---

  /** 删除节点 */
  const handleDeleteNode = async (id: string) => {
    if (!confirm(`⚠ 警告：确认永久删除节点 "${id}" 吗？`)) return;
    try {
        await axios.delete(`${API_URL}/file/${id}`);
        fetchFiles(); 
    } catch (e) { alert("删除失败"); }
  };

  /** 重命名节点 (包含引用更新) */
  const handleRenameNode = async (oldName: string) => {
    const newName = prompt("请输入新名称:", oldName);
    if (!newName || newName === oldName) return;

    try {
        const updates: Promise<void>[] = [];
        
        // 遍历所有节点，更新指向旧名称的引用
        nodes.forEach(node => {
            if (node.id === oldName) return; 

            let modified = false;
            const data = jsyaml.load(node.data.content) as StoryUnitData;
            const end = data.EndCondition;

            if (end?.Type === 'Linear' && end.NextUnitID === oldName) {
                end.NextUnitID = newName;
                modified = true;
            }
            
            if (end?.Branches) {
                Object.keys(end.Branches).forEach(k => {
                    const branch = end.Branches![k];
                    if (typeof branch === 'string' && branch === oldName) {
                        end.Branches![k] = newName;
                        modified = true;
                    } else if (typeof branch === 'object' && branch.NextUnitID === oldName) {
                        branch.NextUnitID = newName;
                        modified = true;
                    }
                });
            }

            if (modified) {
                const newYaml = jsyaml.dump(data, { flowLevel: 3 });
                updates.push(saveFileToBackend(node.id, newYaml));
            }
        });

        await Promise.all(updates);
        await axios.post(`${API_URL}/rename`, { old_name: oldName, new_name: newName });
        fetchFiles();

    } catch (e) {
        console.error(e);
        alert("重命名失败，请检查控制台");
    }
  };

  /** 修改连线样式 */
  const handleEdgeStyle = async (sourceFile: string, handleId: string, key: keyof VisualConfig, value: string) => {
     const node = nodes.find(n => n.id === sourceFile);
     if(!node) return;

     const data = jsyaml.load(node.data.content) as StoryUnitData;
     if(!data.EndCondition) return;
     
     if(!data.EndCondition._Visual) data.EndCondition._Visual = {};
     if(!data.EndCondition._Visual[handleId]) data.EndCondition._Visual[handleId] = {};

     (data.EndCondition._Visual[handleId] as any)[key] = value;

     const newYaml = jsyaml.dump(data, { flowLevel: 3 });
     await saveFileToBackend(sourceFile, newYaml);
     fetchFiles(); 
  };

  /** 删除连线 */
  const handleDeleteEdge = async (sourceFile: string, handleId: string) => {
     if (!confirm(`⚠ 警告：确认删除此连线吗？`)) return;
     
     const node = nodes.find(n => n.id === sourceFile);
     if(!node) return;

     try {
        const data = jsyaml.load(node.data.content) as StoryUnitData;
        if(!data.EndCondition) return;
        
        // 删除连线：根据 handleId 类型处理
        if (handleId === 'next') {
           // 删除 Linear 连接
           if (data.EndCondition.Type === 'Linear') {
              data.EndCondition.NextUnitID = '';
           }
        } else {
           // 删除分支连接
           if (data.EndCondition.Branches && data.EndCondition.Branches[handleId]) {
              delete data.EndCondition.Branches[handleId];
           }
           // 同时删除该分支的视觉配置
           if (data.EndCondition._Visual && data.EndCondition._Visual[handleId]) {
              delete data.EndCondition._Visual[handleId];
           }
        }

        const newYaml = jsyaml.dump(data, { flowLevel: 3 });
        await saveFileToBackend(sourceFile, newYaml);
        fetchFiles();
     } catch (e) {
        alert("删除连线失败");
     }
  };

  // --- 菜单选项配置 ---

  const getNodeOptions = () => [
    { label: '✏️ 重命名 (Rename)', action: () => handleRenameNode(menu!.targetId) },
    { label: '🗑️ 删除 (Delete)', action: () => handleDeleteNode(menu!.targetId) }
  ];

  const getEdgeOptions = (edgeData: { sourceFile: string, handleId: string }) => [
    { label: '🔴 设为红色', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Color', '#ff4444') },
    { label: '🟢 设为绿色', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Color', '#44ff44') },
    { label: '🔵 设为蓝色', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Color', '#4444ff') },
    { label: '⚪ 设为白色', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Color', '#ffffff') },
    { label: '➖ 设为实线', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Style', 'solid') },
    { label: '┄ 设为虚线', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Style', 'dashed') },
    { label: '··· 设为点线', action: () => handleEdgeStyle(edgeData.sourceFile, edgeData.handleId, 'Style', 'dotted') },
    { label: '🗑️ 删除连线', action: () => handleDeleteEdge(edgeData.sourceFile, edgeData.handleId) },
  ];

  // --- 界面渲染 ---
  return (
    <div className="w-screen h-screen bg-black flex flex-col relative" onClick={() => setMenu(null)}>
      
      {/* 全局右键菜单 */}
      {menu && (
          <ContextMenu 
            x={menu.x} y={menu.y} 
            onClose={() => setMenu(null)}
            options={menu.type === 'NODE' ? getNodeOptions() : getEdgeOptions(menu as any)}
          />
      )}

      {/* 
        === 顶部导航栏 === 
        已更新：符合 NEO STUDIO PRO 的视觉设计 (竖线 + 粗体文字)
      */}
      <div className="h-16 border-b border-gemini-border flex items-center px-6 justify-between bg-[#050505] z-10 relative shadow-lg select-none">
        <div className="flex items-center h-full">
          
          {/* 橙色竖线 */}
          <div className="w-[3px] h-5 bg-gemini-orange mr-3"></div>

          {/* Logo 文字组 */}
          <div className="flex items-baseline" style={{ fontFamily: "'Rajdhani', sans-serif" }}>
             <span className="text-2xl font-bold text-white">NEO</span>
             <span className="text-2xl font-bold text-gemini-orange ml-2">STUDIO</span>
             <span className="text-xs font-bold text-gemini-dim ml-3 tracking-[0.3em]">PRO</span> 
          </div>
          
          {/* 分隔符 */}
          <div className="h-8 w-[1px] bg-gemini-border mx-6"></div>
          
          {/* 系统状态 */}
          <div className="flex items-center gap-2 opacity-80">
              <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse shadow-[0_0_5px_#0f0]"></span>
              <span className="text-[10px] text-gemini-dim font-bold tracking-widest font-mono">SYSTEM ONLINE</span>
          </div>
        </div>

        <button 
          onClick={async () => {
             const name = prompt("请输入新单元文件名 (ID):");
             if (!name) return;
             const tpl = `Events:\n  - Type: Narration\n    Mode: Preset\n    Content: "New story..."\nEndCondition:\n  Type: Linear\n  NextUnitID: ""`;
             await saveFileToBackend(name, tpl);
             fetchFiles();
          }}
          className="gemini-btn gemini-btn-primary"
        >
          <PlusCircle size={16} /> NEW UNIT
        </button>
      </div>

      {/* 
        === 画布区域 === 
        已更新：背景使用网格线 (Lines)
      */}
      <div className="flex-1 relative z-0">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onNodeClick={onNodeClick}
          onNodeContextMenu={onNodeContextMenu}
          onEdgeContextMenu={onEdgeContextMenu}
          nodeTypes={nodeTypes}
          fitView
          className="bg-black"
        >
          {/* 网格背景配置: Variant.Lines, 深灰线条 */}
          <Background 
            variant={BackgroundVariant.Lines} 
            color="#1a1a1a" 
            gap={40} 
            size={1} 
            lineWidth={1}
          />
          <Controls className="!bg-black !border-gemini-border !fill-gemini-orange !rounded-none" />
          <MiniMap 
            nodeColor="#ff9900" 
            maskColor="rgba(0, 0, 0, 0.8)" 
            className="!bg-black !border !border-gemini-border !rounded-none"
          />
        </ReactFlow>

        {/* 侧边栏编辑器 */}
        {isEditorOpen && selectedFile && (
          <EditorPanel 
            fileName={selectedFile} 
            content={editorContent} 
            onClose={() => setIsEditorOpen(false)}
            onSave={async (name, content) => { 
                await saveFileToBackend(name, content); 
                fetchFiles(); 
                setIsEditorOpen(false); 
            }}
          />
        )}
      </div>
      
      {/* 底部状态栏 */}
      <div className="absolute bottom-4 left-4 z-10 text-[10px] text-gemini-dim flex gap-4 pointer-events-none font-mono">
        <span className="flex items-center gap-2"><Cpu size={10}/> MEMORY INTEGRITY: 100%</span>
        <span className="opacity-50">|</span>
        <span>SYNC ACTIVE</span>
      </div>
    </div>
  );
}