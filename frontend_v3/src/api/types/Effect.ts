import * as effects from "../../components/effects";
import { SingleAudio } from "../services/UIStatus";

export type IEffectConfig = object;

type Effects = typeof effects.StarField | typeof effects.Rain;

export interface IEffect {
    type: Effects;
    audio?: SingleAudio; //音效
    config?: IEffectConfig; // 如果有需要外部修改的设置，由此导出
}
