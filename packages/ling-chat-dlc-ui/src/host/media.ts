import { onMounted, onBeforeUnmount } from "vue";
import { isOwnedByStandaloneDlc, releaseFolderFromEvent } from "../utils/dlcMediaOwnership";
export function configureExtensionAudio(audio: HTMLAudioElement) {
  audio.preservesPitch = false;
  (audio as HTMLAudioElement & { mozPreservesPitch?: boolean }).mozPreservesPitch = false;
  (audio as HTMLAudioElement & { webkitPreservesPitch?: boolean }).webkitPreservesPitch = false;
}
export function useExtensionAudio(
  sources: () => (HTMLAudioElement | null)[],
  release: (audio: HTMLAudioElement) => void,
) {
  const onRelease = (event: Event) => {
    const folder = releaseFolderFromEvent(event);
    for (const audio of sources()) {
      if (audio && isOwnedByStandaloneDlc(audio.currentSrc || audio.src, folder)) release(audio);
    }
  };
  onMounted(() => window.addEventListener("lingchat:release-dlc-media", onRelease));
  onBeforeUnmount(() => window.removeEventListener("lingchat:release-dlc-media", onRelease));
}
