import send from "../services/message.ts";
import { API_URLS } from "../consts.ts";
import { ModelInfo } from "./ModelInfo.ts";
import { SingleChat, Save } from "./Save.ts";
export interface CharacterAvatars {
    //urls
    happy?: string;
    sad?: string;
    // TODO
}
export interface CharacterCardCover {
    id: number;
    cover: string;
    title: string;
    description: string;
}
export interface CharacterCard {
    cover: CharacterCardCover;
    player_name: string;
    player_subtitle: string;
    ai_name: string;
    ai_subtitle: string;
    ai_model: ModelInfo;
    ai_avatars: CharacterAvatars;
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
                    ai_name: response.data[cover.id].ai_name,
                    ai_subtitle: response.data[cover.id].ai_subtitle,
                    ai_model: response.data[cover.id].ai_model as ModelInfo,
                    ai_avatars: response.data[cover.id].ai_avatars as CharacterAvatars
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
