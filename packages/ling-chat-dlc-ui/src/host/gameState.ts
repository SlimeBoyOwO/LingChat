import type { ScriptChoiceItem } from "@lingchat/host/types/script";
import type { ScriptPoemGameEvent } from "../types/story";
export interface ExtensionGameState {
  forceChoice: {
    requestId: string;
    choices: ScriptChoiceItem[];
    forced: string;
  } | null;
  poemGame: ScriptPoemGameEvent | null;
  preScriptRoleIds: number[] | null;
}
export const extensionGameState = (): ExtensionGameState => ({
  forceChoice: null,
  poemGame: null,
  preScriptRoleIds: null,
});
