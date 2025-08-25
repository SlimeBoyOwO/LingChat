import { UIStatus, createUIStatusStatic } from "./services/UIStatus.ts";
import {
  UserInfo,
  UserData,
  createUserInfoStatic,
  createUserDataStatic,
} from "./services/UserInfo.ts";
import {
  Settings,
  Defaults,
  createSettingsStatic,
  createDefaultsStatic,
} from "./services/Settings.ts";
import { ref, Ref } from "vue";

type UIController = {
  main: UIStatus;
  settings: UIStatus;
};

export const uiStatus: Ref<UIController> = ref({
  main: createUIStatusStatic("MenuView"),
  settings: createUIStatusStatic("text"),
});
export const userInfo: Ref<UserInfo> = ref(createUserInfoStatic());
export const userData: Ref<UserData> = ref(createUserDataStatic());
export const settings: Ref<Settings> = ref(createSettingsStatic());
export const defaults: Ref<Defaults> = ref(createDefaultsStatic());
