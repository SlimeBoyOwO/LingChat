import { IconType } from "../../components/controls/Icon.vue";
import { CustomControl } from "./CustomControls";

type HTMLElementName = string;
export type SettingPage = {
    element: HTMLElementName | CustomControl;
    bindings: {
        title: string;
        show: boolean;
        [property: string]: unknown;
    };
};

//一级菜单
export type SettingNode1 = {
    [title: string]: { icon: IconType; content: SettingPage[] | SettingNode2 };
};

//二级菜单
export type SettingNode2 = {
    [title: string]: { icon: IconType; content: SettingPage[] };
};
//为了保证界面清晰，只生成到二级菜单，不接受更多级菜单
