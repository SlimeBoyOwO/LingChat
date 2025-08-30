export interface IEffect {
    root?: HTMLCanvasElement;
    initialize: (root: HTMLCanvasElement) => this;
}

export const IEffectEmpty: IEffect = {
    root: undefined,
    initialize(root: HTMLCanvasElement) {
        this.root = root;
        return this;
    }
};
