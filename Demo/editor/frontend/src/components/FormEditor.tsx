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