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