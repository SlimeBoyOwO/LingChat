import { SingleAudio } from "../services/UIStatus";

export interface IEffect {
    _root?: HTMLCanvasElement; //根元素，在此元素上进行效果绘制
    audio?: SingleAudio; //音效
    initialize: (root: HTMLCanvasElement) => this;
}

export const IEffectEmpty: IEffect = {
    initialize(root: HTMLCanvasElement) {
        this._root = root;
        return this;
    }
};
