import { IBubble } from "../types/Bubble.ts";
import { SingleAvatar } from "../types/CharacterCard.ts";

export enum AIStatus {
    IDLE,
    THINKING,
    RESPONDING,
    FAILED
}

export interface GameStatus {
    current: {
        avatar?: SingleAvatar;
        status: AIStatus;
        bubble?: IBubble
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
