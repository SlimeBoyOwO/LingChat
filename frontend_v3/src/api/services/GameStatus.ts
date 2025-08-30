import { IBubble } from "../types/Bubble.ts";
import { Emotion, SingleAvatar } from "../types/CharacterCard.ts";

export enum AIStatus {
    IDLE,
    THINKING,
    RESPONDING,
    FAILED
}

export interface GameStatus {
    current: {
        status: AIStatus;
        name: string;
        subtitle: string;
        emotion: Emotion;
        placeholder: string;
        text: string;
        avatar?: SingleAvatar;
        bubble?: IBubble;
    };
}

export function createGameStatusStatic(): GameStatus {
    return {
        current: {
            avatar: undefined,
            status: AIStatus.IDLE
        }
    };
}
