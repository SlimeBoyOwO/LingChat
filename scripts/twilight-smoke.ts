// Development-only harness for the controller mounted by TwilightRhythm.vue.
import { mountRhythm } from "../src/minigames/twilight/game.js";
import template from "../src/minigames/twilight/template.html?raw";
import styles from "../src/minigames/twilight/style.css?raw";
import "../src/assets/styles/base.css";
const root = document.getElementById("rhythm-host")!.attachShadow({ mode: "open" });
root.innerHTML = template;
const style = document.createElement("style");
style.textContent = styles;
root.prepend(style);
const lifetime = new AbortController();
(window as any).__rhythmController = await mountRhythm(root, {
  signal: lifetime.signal,
  onExit: () => {
    (window as any).__rhythmExited = true;
  },
});
(window as any).__rhythmAbort = () => lifetime.abort();
