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