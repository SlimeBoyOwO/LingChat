/** Independent pointer sources allow chords and moving, jumping and firing together. */
export function bindTouchControls(root, { selector, enabled, press, release, signal, slide = [] }) {
  const buttons = [...root.querySelectorAll(selector)];
  const pointers = new Map();
  const on = (target, type, handler) => target.addEventListener(type, handler, { signal });
  const key = (button) => button.dataset.action ?? Number(button.dataset.lane);
  const end = (id) => {
    const held = pointers.get(id);
    if (!held) return;
    pointers.delete(id);
    release(key(held.button), `pointer-${id}`);
    if (![...pointers.values()].some((entry) => entry.button === held.button))
      held.button.classList.remove("active");
    if (held.capture.hasPointerCapture(id)) held.capture.releasePointerCapture(id);
  };
  for (const button of buttons) {
    on(button, "contextmenu", (event) => event.preventDefault());
    on(button, "pointerdown", (event) => {
      if (!enabled() || (event.pointerType === "mouse" && event.button !== 0)) return;
      event.preventDefault();
      button.setPointerCapture(event.pointerId);
      pointers.set(event.pointerId, { button, capture: button });
      button.classList.add("active");
      press(key(button), `pointer-${event.pointerId}`);
    });
    // A thumb may slide between the two direction keys without being lifted.
    on(button, "pointermove", (event) => {
      const held = pointers.get(event.pointerId);
      if (!held || !slide.includes(key(held.button))) return;
      const next = buttons.find((candidate) => {
        if (!slide.includes(key(candidate))) return false;
        const box = candidate.getBoundingClientRect();
        return (
          event.clientX >= box.left &&
          event.clientX <= box.right &&
          event.clientY >= box.top &&
          event.clientY <= box.bottom
        );
      });
      if (!next || next === held.button) return;
      const old = held.button;
      held.button = next;
      release(key(old), `pointer-${event.pointerId}`);
      if (![...pointers.values()].some((entry) => entry.button === old))
        old.classList.remove("active");
      next.classList.add("active");
      press(key(next), `pointer-${event.pointerId}`);
    });
    for (const type of ["pointerup", "pointercancel", "lostpointercapture"])
      on(button, type, (event) => end(event.pointerId));
  }
  const clear = () => {
    for (const id of [...pointers.keys()]) end(id);
    buttons.forEach((button) => button.classList.remove("active"));
  };
  signal.addEventListener("abort", clear, { once: true });
  return { clear };
}
