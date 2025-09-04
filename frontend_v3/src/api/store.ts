import { computed, ComputedRef, ref, Ref } from "vue";

import { Slider } from "../components/controls/index.ts";
import { createGameStatusStatic, GameStatus } from "./services/GameStatus.ts";
import { createI18NStatic, I18N } from "./services/I18N.ts";
import { createDefaultsStatic, createSettingsStatic, Defaults, Settings } from "./services/Settings.ts";
import { createUIStatusStatic, extendToRootUIStatus, RootUIStatus, UIStatus } from "./services/UIStatus.ts";
import { createUserDataStatic, createUserInfoStatic, UserData, UserInfo } from "./services/UserInfo.ts";
import { SettingNode1 } from "./types/settings.ts";

type UIController = {
    main: RootUIStatus;
    settings: UIStatus;
};

export const uiStatus: Ref<UIController> = ref({
    main: extendToRootUIStatus(createUIStatusStatic("MenuView")),
    settings: createUIStatusStatic("text")
});
export const userInfo: Ref<UserInfo> = ref(createUserInfoStatic());
export const userData: Ref<UserData> = ref(createUserDataStatic());
export const settings: Ref<Settings> = ref(createSettingsStatic());
export const defaults: Ref<Defaults> = ref(createDefaultsStatic());
export const gameStatus: Ref<GameStatus> = ref(createGameStatusStatic());

const INTERNATIONALIZATION: Ref<I18N> = ref(createI18NStatic());
export const i18n: ComputedRef<(key: string) => string> = computed(
    () => (key: string) => INTERNATIONALIZATION.value.get(key)
);
// export const settings_structure: SettingNode1 = await getSettingsStructure();

export const settings_structure: SettingNode1 = <SettingNode1>{
    text: {
        icon: "character",
        content: [
            {
                element: Slider,
                bindings: {
                    title: "Text Speed",
                    value: settings.value.text_speed,
                    show: true
                }
            }
        ]
    },
    text2: {
        icon: "character",
        content: {
            text: {
                icon: "character",
                content: [
                    {
                        element: Slider,
                        bindings: {
                            title: "Text Speed",
                            value: settings.value.text_speed,
                            show: true
                        }
                    }
                ]
            }
        }
    }
};
