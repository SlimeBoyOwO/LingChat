// Development-only harness for the same controller, template and styles used by StarTrail.vue.
import { mountStarTrail } from "../src/minigames/star-trail/game.js";
import template from "../src/minigames/star-trail/template.html?raw";
import styles from "../src/minigames/star-trail/style.css?raw";
import "../src/assets/styles/base.css";
const root = document.getElementById("trail-host")!.attachShadow({ mode: "open" });
root.innerHTML = template;
const style = document.createElement("style");
style.textContent = styles;
root.prepend(style);
const lifetime = new AbortController();
const controller = mountStarTrail(root, {
  signal: lifetime.signal,
  onExit: () => {
    (window as any).__trailExited = true;
  },
});
(window as any).__trailController = controller;
(window as any).__trailAbort = () => lifetime.abort();
