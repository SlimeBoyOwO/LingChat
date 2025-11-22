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