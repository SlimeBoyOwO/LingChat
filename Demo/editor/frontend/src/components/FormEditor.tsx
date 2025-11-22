/**
 * FormEditor.tsx
 * 
 * 可视化表单编辑器组件
 * 功能：
 * 1. 编辑剧情事件列表 (支持拖拽排序、添加、删除)
 * 2. 编辑流程控制 (跳转条件、分支管理)
 */

import React, { useRef } from 'react';
import { Trash2, Plus, Layers, GripVertical } from 'lucide-react';
import type { StoryUnitData } from '../types';

interface FormEditorProps {
  data: StoryUnitData;
  onChange: (newData: StoryUnitData) => void;
}

export const FormEditor: React.FC<FormEditorProps> = ({ data, onChange }) => {
  
  // --- 拖拽排序 Ref ---
  const dragItem = useRef<number | null>(null); // 当前拖动的项目索引
  const dragOverItem = useRef<number | null>(null); // 拖动经过的目标索引

  // --- 事件操作函数 ---

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

  // --- 拖拽处理逻辑 ---

  const handleDragStart = (e: React.DragEvent, position: number) => {
    dragItem.current = position;
    e.dataTransfer.effectAllowed = "move";
    // 视觉反馈：半透明
    const target = e.currentTarget as HTMLElement;
    target.style.opacity = "0.5";
  };

  const handleDragEnter = (e: React.DragEvent, position: number) => {
    dragOverItem.current = position;
    e.preventDefault(); // 允许 Drop
  };

  const handleDragEnd = (e: React.DragEvent) => {
    const target = e.currentTarget as HTMLElement;
    target.style.opacity = "1"; // 恢复不透明

    if (dragItem.current !== null && dragOverItem.current !== null && dragItem.current !== dragOverItem.current) {
      const newEvents = [...(data.Events || [])];
      const draggedItemContent = newEvents[dragItem.current];
      
      // 移动数组元素
      newEvents.splice(dragItem.current, 1);
      newEvents.splice(dragOverItem.current, 0, draggedItemContent);
      
      onChange({ ...data, Events: newEvents });
    }
    // 重置指针
    dragItem.current = null;
    dragOverItem.current = null;
  };

  // --- 流程控制操作函数 ---

  const updateEndType = (type: string) => {
    const newEnd = { ...data.EndCondition, Type: type as any };
    // 切换类型时重置必要字段
    if (type === 'Linear' && !newEnd.NextUnitID) newEnd.NextUnitID = '';
    if (type !== 'Linear' && !newEnd.Branches) newEnd.Branches = { 'A': '', 'B': '' };
    onChange({ ...data, EndCondition: newEnd });
  };

  /** 更新分支：支持修改分支指向的目标 ID */
  const updateBranchTarget = (key: string, targetId: string) => {
     const newBranches = { ...(data.EndCondition.Branches || {}) };
     const original = newBranches[key];
     
     // 兼容性处理：保留可能存在的对象结构
     if (typeof original === 'object' && original !== null) {
        newBranches[key] = { ...original, NextUnitID: targetId };
     } else {
        newBranches[key] = targetId;
     }
     onChange({ ...data, EndCondition: { ...data.EndCondition, Branches: newBranches } });
  };

  /** 添加新分支 Key */
  const addBranch = () => {
    const newKey = prompt("输入新选项 Key (例如: OPTION_C):", "C");
    if (newKey) updateBranchTarget(newKey, "");
  }

  /** 删除分支 Key */
  const removeBranch = (key: string) => {
      if(!confirm(`确定删除分支 "${key}" 吗？`)) return;
      const newBranches = { ...(data.EndCondition.Branches || {}) };
      delete newBranches[key];
      onChange({ ...data, EndCondition: { ...data.EndCondition, Branches: newBranches } });
  }

  /** 重命名分支 Key */
  const renameBranch = (oldKey: string) => {
      const newKey = prompt("重命名 Key 为:", oldKey);
      if(!newKey || newKey === oldKey) return;

      const branches = data.EndCondition.Branches || {};
      const newBranches: Record<string, any> = {};

      // 重构对象以保持顺序（虽然 JS 对象不保证顺序，但通常有效）
      Object.keys(branches).forEach(k => {
          if (k === oldKey) {
              newBranches[newKey] = branches[oldKey]; // 转移值到新 Key
          } else {
              newBranches[k] = branches[k];
          }
      });
      
      onChange({ ...data, EndCondition: { ...data.EndCondition, Branches: newBranches } });
  }

  return (
    <div className="space-y-8 pb-10 font-mono">
      
      {/* === 1. 剧情事件列表 (Story Events) === */}
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
             <div className="text-center py-8 text-gemini-dim text-xs italic border border-dashed border-gemini-border bg-black/50">
               暂无事件，点击上方添加...
             </div>
          )}

          {data.Events?.map((ev, idx) => (
            <div 
              key={idx} 
              draggable
              onDragStart={(e) => handleDragStart(e, idx)}
              onDragEnter={(e) => handleDragEnter(e, idx)}
              onDragOver={(e) => e.preventDefault()} // 必须阻止默认行为以允许 Drop
              onDragEnd={handleDragEnd}
              className="bg-gemini-panel border border-gemini-border p-3 rounded hover:border-gemini-orange/50 transition-all relative group cursor-move"
            >
              {/* 拖拽手柄图标 */}
              <div className="absolute left-2 top-1/2 -translate-y-1/2 text-gemini-dim opacity-20 group-hover:opacity-50 cursor-grab active:cursor-grabbing">
                 <GripVertical size={16} />
              </div>

              {/* 删除按钮 (悬浮显示) */}
              <div className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity bg-black/50 backdrop-blur rounded p-1 z-10">
                <button onClick={() => removeEvent(idx)} className="p-1 text-gemini-dim hover:text-red-500 transition-colors"><Trash2 size={12}/></button>
              </div>

              <div className="pl-6"> {/* 左边距留给手柄 */}
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
            </div>
          ))}
        </div>
      </div>

      {/* === 2. 流程控制 (Flow Control) === */}
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

          {/* --- 线性模式 --- */}
          {(data.EndCondition?.Type === 'Linear') && (
            <div>
              <label className="gemini-label">NEXT UNIT ID (TARGET)</label>
              <input 
                type="text" 
                disabled
                value={data.EndCondition.NextUnitID || ''} 
                className="gemini-input text-gemini-dim cursor-not-allowed bg-gemini-panel/50 border-dashed"
                placeholder="请在画布上拖拽连线..."
              />
              <p className="text-[10px] text-gemini-orange mt-2 flex items-center gap-1">
                <span className="animate-pulse">●</span> 在画布连线可自动填充此处
              </p>
            </div>
          )}

          {/* --- 分支模式 --- */}
          {['Branching', 'AIChoice', 'PlayerResponseBranch'].includes(data.EndCondition?.Type || '') && (
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                 <label className="gemini-label">BRANCHES (OUTLETS)</label>
                 <button onClick={addBranch} className="text-gemini-blue hover:text-white text-[10px] flex items-center gap-1 hover:underline"><Plus size={10}/> ADD KEY</button>
              </div>
              
              {Object.keys(data.EndCondition?.Branches || {}).map((key) => {
                 const val = data.EndCondition!.Branches![key];
                 const target = typeof val === 'object' ? val.NextUnitID : val;
                 
                 return (
                   <div key={key} className="flex items-center gap-2 group">
                     {/* 分支 Key (可点击重命名) */}
                     <div 
                        className="w-24 text-right font-mono text-xs text-gemini-blue font-bold truncate cursor-pointer hover:text-white hover:underline" 
                        title="点击重命名 Key"
                        onClick={() => renameBranch(key)}
                     >
                        {key}
                     </div>
                     
                     <div className="text-gemini-dim">→</div>
                     
                     {/* 目标 ID (只读) */}
                     <input 
                       type="text" 
                       readOnly
                       value={target || '未连接'} 
                       className="gemini-input flex-1 text-xs text-gemini-dim border-none bg-gemini-bg"
                     />
                     
                     {/* 删除分支按钮 */}
                     <button className="text-gemini-dim hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity p-1">
                        <Trash2 size={12} onClick={() => removeBranch(key)}/>
                     </button>
                   </div>
                 )
              })}
              <p className="text-[10px] text-gemini-dim mt-1 border-t border-gemini-border/50 pt-2 italic">
                提示：点击左侧蓝色 Key 可重命名。
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};