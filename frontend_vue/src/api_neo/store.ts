import { UIStatus, initUIStatus } from "./services/UIStatus.ts";
import { UserInfo, initUserInfo } from "./services/UserInfo.ts";
import { Settings, Globals, initSettings, initGlobals } from "./services/Settings.ts";

const uiStatus: UIStatus = await initUIStatus();
const userInfo: UserInfo = await initUserInfo();
const settings: Settings = await initSettings();
const globals: Globals = await initGlobals();
