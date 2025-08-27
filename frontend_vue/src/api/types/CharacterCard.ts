import { API_URLS } from "../consts.ts";
import { send } from "../services/message.ts";
import { ModelInfo } from "./ModelInfo.ts";
import { Save, SingleChat } from "./Save.ts";

export type SingleAvatar = string;
type AvatarAudioUrl = string;

interface Emotions<T> {
    //urls
    happy?: T;
    sad?: T;
    angry?: T;
    // TODO
}

export interface CharacterAvatars extends Emotions<SingleAvatar> {
    audios: Emotions<AvatarAudioUrl>;
}

export interface CharacterCardCover {
    id: number;
    cover: string;
    title: string;
    description: string;
}
type SingleAICharacter = {
    name: string;
    subtitle: string;
    model: ModelInfo;
    avatars: CharacterAvatars;
};
export interface CharacterCard {
    cover: CharacterCardCover;
    player_name: string;
    player_subtitle: string;
    ai: SingleAICharacter[];
    history16: SingleChat[];
    save: Save;
}
export async function getCharacterCardCover(card_id: number[]): Promise<CharacterCardCover[]> {
    return send(API_URLS.CARD.CHARACTER.COVER, {
        id: card_id
    }).then(response => {
        return Array.from<CharacterCardCover>(response.data.values());
    });
}

export async function getCharacterCardExtend(covers: CharacterCardCover[]): Promise<CharacterCard[]> {
    return send(API_URLS.CARD.CHARACTER.EXTEND, {
        id: covers.map(cover => cover.id)
    }).then(response => {
        return covers.map(
            cover =>
                <CharacterCard>{
                    cover: cover,
                    player_name: response.data[cover.id].player_name,
                    player_subtitle: response.data[cover.id].player_subtitle,
                    ai: response.data[cover.id].ai as SingleAICharacter[]
                }
        );
    });
}

export async function getCharacterCardFull(card_id: number[]): Promise<CharacterCard[]> {
    return send(API_URLS.CARD.CHARACTER.SINGLE, {
        id: card_id
    }).then(response => {
        return Array.from<CharacterCard>(response.data.values());
    });
}

export async function searchCharacterCards(card_name: string): Promise<CharacterCardCover[]> {
    return send(API_URLS.CARD.CHARACTER.SEARCH, {
        name: card_name
    }).then(response => {
        return Array.from<CharacterCardCover>(response.data.values());
    });
}
