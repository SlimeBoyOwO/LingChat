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

      // 重构对象以保持顺序
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
      
      {/* === 1. Story Events === */}
      <div className="space-y-4">
        <div className="flex items-center justify-between border-b border-neo-border pb-2">
          <h3 className="text-neo-main font-display font-bold text-sm tracking-[0.2em] flex items-center gap-2">
            <Layers size={14} /> EVENT SEQUENCE
          </h3>
          <button onClick={addEvent} className="neo-btn neo-btn-ghost py-1 px-2 text-[10px]">
            <Plus size={12} /> ADD ENTRY
          </button>
        </div>

        <div className="space-y-3">
          {(!data.Events || data.Events.length === 0) && (
             <div className="text-center py-8 text-neo-dim text-xs italic border border-dashed border-neo-border bg-neo-bg/30">
               // NO EVENTS DETECTED.
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
              className="bg-neo-bg border border-neo-border p-3 neo-bracket group cursor-move hover:border-neo-main transition-colors"
            >
              {/* 删除按钮 (悬浮显示) */}
              <div className="absolute right-2 top-2 opacity-0 group-hover:opacity-100 transition-opacity z-10">
                <button onClick={() => removeEvent(idx)} className="p-1 text-neo-dim hover:text-red-500 transition-colors"><Trash2 size={12}/></button>
              </div>

              <div className="flex gap-3">
                {/* 拖拽手柄图标 */}
                <div className="pt-2 text-neo-dim opacity-30 group-hover:opacity-80 cursor-grab active:cursor-grabbing">
                   <GripVertical size={16} />
                </div>

                <div className="flex-1 space-y-3">
                    <div className="grid grid-cols-12 gap-2">
                        <div className="col-span-5">
                            <label className="neo-label">TYPE</label>
                            <select value={ev.Type} onChange={(e) => updateEvent(idx, 'Type', e.target.value)} className="neo-select">
                            <option value="Narration">NARRATION</option>
                            <option value="Dialogue">DIALOGUE</option>
                            <option value="Player">PLAYER</option>
                            <option value="Action">ACTION</option>
                            <option value="SystemAction">SYSTEM</option>
                            </select>
                        </div>
                        <div className="col-span-4">
                            <label className="neo-label">MODE</label>
                            <select value={ev.Mode || 'Preset'} onChange={(e) => updateEvent(idx, 'Mode', e.target.value)} className="neo-select">
                            <option value="Preset">PRESET</option>
                            <option value="Prompt">PROMPT</option>
                            <option value="Input">INPUT</option>
                            </select>
                        </div>
                        {ev.Type === 'Dialogue' && (
                            <div className="col-span-3">
                            <label className="neo-label">ID</label>
                            <input type="text" value={ev.Character || ''} onChange={(e) => updateEvent(idx, 'Character', e.target.value)} className="neo-input text-center" placeholder="ID" />
                            </div>
                        )}
                    </div>

                    <div>
                    <label className="neo-label">CONTENT / PAYLOAD</label>
                    <textarea 
                        rows={ev.Mode === 'Prompt' ? 4 : 2}
                        value={ev.Content || ''}
                        onChange={(e) => updateEvent(idx, 'Content', e.target.value)}
                        className="neo-input resize-none leading-relaxed text-xs"
                        placeholder={ev.Mode === 'Prompt' ? "Input Prompt Instruction..." : "Input Text Content..."}
                    />
                    </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* === 2. Flow Control (Flow Control) === */}
      <div className="space-y-4 pt-4">
        <div className="border-b border-neo-border pb-2">
          <h3 className="text-neo-sub font-display font-bold text-sm tracking-[0.2em]">LOGIC CONTROL</h3>
        </div>

        <div className="bg-neo-bg/30 p-4 border border-neo-border border-l-4 border-l-neo-sub">
          <label className="neo-label">EXIT PROTOCOL</label>
          <select 
            value={data.EndCondition?.Type || 'Linear'} 
            onChange={(e) => updateEndType(e.target.value)}
            className="neo-select mb-4 text-neo-sub font-bold"
          >
            <option value="Linear">➔ LINEAR</option>
            <option value="Branching">⑂ BRANCHING (USER)</option>
            <option value="AIChoice">🤖 AI DECISION</option>
            <option value="PlayerResponseBranch">💬 RESPONSE EVAL</option>
          </select>

          {/* --- 线性模式 --- */}
          {(data.EndCondition?.Type === 'Linear') && (
            <div>
              <label className="neo-label">TARGET UNIT ID</label>
              <input 
                type="text" 
                disabled
                value={data.EndCondition.NextUnitID || ''} 
                className="neo-input text-neo-dim border-dashed cursor-not-allowed bg-neo-bg/50"
                placeholder="LINK ON CANVAS..."
              />
              <p className="text-[10px] text-neo-main mt-2 flex items-center gap-1">
                <span className="animate-pulse">●</span> LINK NODES ON CANVAS TO AUTO-FILL
              </p>
            </div>
          )}

          {/* --- 分支模式 --- */}
          {['Branching', 'AIChoice', 'PlayerResponseBranch'].includes(data.EndCondition?.Type || '') && (
            <div className="space-y-3">
              <div className="flex justify-between items-center">
                 <label className="neo-label">OUTLET KEYS</label>
                 <button onClick={addBranch} className="text-neo-sub hover:text-white text-[10px] flex items-center gap-1 hover:underline"><Plus size={10}/> ADD</button>
              </div>
              
              {Object.keys(data.EndCondition?.Branches || {}).map((key) => {
                 const val = data.EndCondition!.Branches![key];
                 const target = typeof val === 'object' ? val.NextUnitID : val;
                 
                 return (
                   <div key={key} className="flex items-center gap-2 group">
                     {/* 分支 Key (可点击重命名) */}
                     <div 
                        className="w-24 text-right font-mono text-xs text-neo-sub font-bold truncate cursor-pointer hover:text-neo-text hover:underline" 
                        title="Click to Rename"
                        onClick={() => renameBranch(key)}
                     >
                        {key}
                     </div>
                     
                     <div className="text-neo-dim">→</div>
                     
                     {/* 目标 ID (只读) */}
                     <input 
                       type="text" 
                       readOnly
                       value={target || 'NULL'} 
                       className="neo-input flex-1 text-xs text-neo-dim border-none bg-neo-bg/50"
                     />
                     
                     {/* 删除分支按钮 */}
                     <button className="text-neo-dim hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity p-1">
                        <Trash2 size={12} onClick={() => removeBranch(key)}/>
                     </button>
                   </div>
                 )
              })}
              <p className="text-[10px] text-neo-dim mt-1 border-t border-neo-border/50 pt-2 italic">
                HINT: Click Blue Keys to Rename.
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};