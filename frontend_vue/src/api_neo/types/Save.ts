import { LIMITS } from "../consts";
import { CharacterCard } from "./CharacterCard";

export interface SingleSave {
    id: number;
    save_time: Date;
    title: string;
    description: string;
}

export interface Save {
    __saves: SingleSave[];
    readonly latest: SingleSave;
    readonly count: number;
    add: (save: SingleSave) => void;
    remove: { (index: number): void; (item: SingleSave): void };
}

export interface SingleChat {
    id: number;
    user?: string;
    ai: string[];
}

export function newSave(saves: SingleSave[] = []): Save {
    return <Save>{
        __saves: saves,
        get latest() {
            return this.__saves[this.__saves.length - 1];
        },
        get count() {
            return this.__saves.length;
        },
        add(save: SingleSave) {
            if (this.count >= LIMITS.MAX_SAVE_COUNT) this.__saves.push(save);
        },
        remove(param: number | SingleSave) {
            if (typeof param === "number") {
                const index: number = param;
                if (param < 0 || this.count <= param) {
                    throw new Error(`Index(${index}) out of range(0~${this.count - 1})`);
                }
                this.__saves.splice(index, 1);
            } else {
                const item: SingleSave = param;
                const index: number = this.__saves.indexOf(item);
                if (index === -1) {
                    throw new Error(`${item} not found in saves`);
                }
                this.__saves.splice(index, 1);
            }
        }
    };
}

export function newSingleSave(card: CharacterCard, chat: SingleChat): SingleSave {
    return <SingleSave>{
        id: card.save.count,
        save_time: new Date(),
        title: chat.user ? chat.user.slice(0, 20) : `Save ${card.save.count + 1}`,
        description: chat.ai[0].slice(0, 20)
    };
}
