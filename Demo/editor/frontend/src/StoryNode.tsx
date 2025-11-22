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

  // 动态端口颜色：使用 Tailwind 的 text/bg 类让其自动适配 CSS 变量
  const mainColorClass = "bg-neo-main text-neo-main";
  const subColorClass = "bg-neo-sub text-neo-sub";

  return (
    <div className={`
      neo-bracket min-w-[220px] bg-neo-bg border transition-all duration-300 group
      ${selected ? 'border-neo-main shadow-neo z-10' : 'border-neo-border opacity-90 hover:opacity-100'}
    `}>
      {/* 顶部标题栏 */}
      <div className={`
        px-3 py-1.5 text-sm font-bold flex items-center gap-2 border-b border-neo-border font-display tracking-wide
        ${selected ? 'bg-neo-main text-neo-bg' : 'bg-neo-panel text-neo-text'}
      `}>
        <FileCode size={14} />
        <span className="truncate">{data.label}</span>
      </div>

      {/* 内容概览 */}
      <div className="p-3 text-[10px] text-neo-dim font-mono bg-neo-bg/50 backdrop-blur-sm">
        <div className="flex justify-between items-center mb-1">
          <span className="uppercase tracking-wider">EXIT TYPE</span>
          <span className={`px-1 font-bold ${endType !== 'Linear' ? 'text-neo-sub' : 'text-neo-main'}`}>
            {endType.toUpperCase()}
          </span>
        </div>
        <div className="border-t border-dashed border-neo-border mt-1 pt-1 flex justify-between">
           <span>EVENTS: {parsedData.Events?.length || 0}</span>
        </div>
      </div>

      {/* 输入锚点 (左侧) */}
      <Handle type="target" position={Position.Left} className={`!w-2 !h-2 !-left-1 !rounded-none ${mainColorClass}`} />

      {/* 输出锚点 (右侧) - 动态生成 */}
      {endType === 'Linear' || endType === 'Conditional' ? (
        <div className="relative">
            <div className={`absolute -right-4 top-[-25px] text-[9px] font-bold ${selected ? 'text-neo-main' : 'text-neo-dim'} opacity-0 group-hover:opacity-100 transition-opacity`}>NEXT</div>
            <Handle type="source" position={Position.Right} id="next" className={`!w-2 !h-2 !-right-1 !rounded-none ${mainColorClass}`} />
        </div>
      ) : (
        <div className="flex flex-col gap-3 py-2 relative">
          {branches.map((branchKey) => (
            <div key={branchKey} className="relative h-4">
              <span className="absolute right-3 text-[9px] text-neo-sub top-0.5 uppercase font-bold text-right w-20 truncate">{branchKey}</span>
              <Handle 
                type="source" 
                position={Position.Right} 
                id={branchKey} 
                style={{ top: '50%' }}
                className={`!w-2 !h-2 !-right-1 !rounded-none ${subColorClass}`} 
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

export default memo(StoryNode);