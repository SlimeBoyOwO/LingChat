export interface StoryEvent {
    Type: 'Narration' | 'Dialogue' | 'Player' | 'Action' | 'SystemAction' | 'FreeTime';
    Mode?: 'Preset' | 'Prompt' | 'Input';
    Character?: string; 
    Content?: string;   
    [key: string]: any;
}

export interface VisualConfig {
    Style?: string; // solid, dashed, dotted
    Color?: string;
    Animated?: boolean;
}

export interface EndCondition {
    Type: 'Linear' | 'Branching' | 'AIChoice' | 'PlayerResponseBranch' | 'Conditional';
    NextUnitID?: string;
    Branches?: Record<string, any>; 
    _Visual?: Record<string, VisualConfig>; 
}

export interface StoryUnitData {
    Events: StoryEvent[];
    EndCondition: EndCondition;
}