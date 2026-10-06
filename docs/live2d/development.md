# Live2D Development Guide

This document describes the implementation contracts for LingChat's optional Live2D layer. Read the [package tutorial](authoring.md) before changing the import schema or character settings.

## Scope and Ownership

Live2D augments the existing character renderer. It does not own:

- emotion classification or the dialogue protocol;
- LLM, TTS, voice fetching, or audio playback;
- static PNG/WebP rendering and cross-fades;
- dialogue bubbles, touch layers, desktop pet dragging, or window management;
- character identity or outfit semantics.

The existing systems remain active. A static avatar is hidden only after a Live2D model loads and renders its first frame successfully.

## Main Components

| Path                                                      | Responsibility                                                                                                                                       |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/game/live2d/Live2DStage.vue`              | Stage ownership, role synchronization, model lifecycle, runtime-result ownership, layout, expressions, motions, gaze, and lip sync integration       |
| `src/components/game/live2d/live2d-stage-context.ts`      | Stage-local, read-only ready/unavailable role results for avatar fallback rendering                                                                  |
| `src/components/game/live2d/live2d-runtime.ts`            | Cubism Core and Pixi Live2D engine loading                                                                                                           |
| `src/components/game/standard/GameRoleAvatar.vue`         | Role-intent dispatch plus shared avatar resolution, layout, animation, bubbles, touch, and effect audio                                              |
| `src/components/game/standard/StaticRolePresentation.vue` | Traditional static image transition and load completion contract                                                                                     |
| `src/components/game/standard/Live2DRolePresentation.vue` | Stage-result consumption, static fallback visibility, and localized unavailable result                                                               |
| `src/components/game/live2d/model-source.ts`              | Safe model3 reference rewriting, loose-asset injection, and configured idle projection                                                               |
| `src/components/game/live2d/live2d-interaction.ts`        | Pointer coordinate and gaze calculations                                                                                                             |
| `src/components/game/live2d/live2d-emotion.ts`            | Emotion-key priority and the resulting expression lookup shared by the runtime and settings UI                                                       |
| `src/components/game/live2d/live2d-touch.ts`              | Drawable-bounds touch regions and stroke-gesture detection                                                                                           |
| `src/components/game/live2d/useLive2dTouch.ts`            | Touch runtime: pointer listeners, sway ownership, eye lid, gaze weight, touch sound, and particle pacing                                             |
| `src/components/game/live2d/TouchParticles.vue`           | Stroke-trail motes drawn over the model during a touch                                                                                               |
| `src/components/game/live2d/live2d-layout.ts`             | Pure layout calculations shared with tests                                                                                                           |
| `src/components/game/live2d/live2d-motion.ts`             | Motion start/finish attribution through engine lifecycle events                                                                                      |
| `src/components/game/live2d/useLive2dLipSync.ts`          | Passive audio decoding and mouth amplitude sampling                                                                                                  |
| `src/components/settings/character/Live2DSettings.vue`    | Import, variant editing, bindings, outfit mapping, and preview                                                                                       |
| `crates/ling-chat-main/src/api/live2d.rs`                 | Directory/ZIP import, inspection, loose-asset discovery, validation, staging, rollback, and runtime refresh                                          |
| `crates/ling-chat-main/src/ai_service/types.rs`           | Serialized `settings.yml.live2d` contract, plus the `avatar_mode` / `avatar_mode_p` display-mode keys and the `pet_frameless` pet-window chrome flag |

## Render Stack

Each mounted `GameRolesStage` creates at most one Pixi `Application`. All Live2D roles in that stage share it.

`Live2DStage` owns model readiness and unavailability results. In standard mode, role DOM is rendered through its default slot. `GameRoleAvatar` chooses `Live2DRolePresentation` or `StaticRolePresentation` through `prefersLive2d(role, "standard")` (`src/types/live2d.ts`), which is true when `role.live2d` exists **and** the character's `settings.yml` `avatar_mode` is not `"image"`. Pet mode makes the same call with `"pet"` against `avatar_mode_p`. `Live2DStage.syncRoles` is the only place that loads models, and it applies the same predicate with its `mode` prop, so a role set to `image` never creates a model or a Pixi `Application`. The Live2D presentation consumes read-only results from the nearest stage context and reuses the static presentation for fallback. `GameRolesStage` must not copy model lifecycle results into its own state or choose a role's presentation from those results. Readiness only controls when the static fallback can be hidden. Display mode is user intent and is deliberately independent of model lifecycle results — never derive it from load success or failure.

The context is local to one mounted stage. Do not turn it into a global store, event bus, role registry, or persistence mechanism.

The intended visual order is:

```text
static character images
Live2D canvas
character bubbles and touch/interaction layers
```

Do not migrate static characters into Pixi to solve a Live2D issue. Mixed static and Live2D scenes are supported intentionally.

## Load Lifecycle

1. Resolve the active variant from `default_variant` and `clothes_variants`.
2. Ask Tauri for the model3 path with `get_live2d_file`, and for the variant's loose-asset table with `get_live2d_variant_assets` in parallel.
3. Fetch model3 through Tauri's asset protocol.
4. Merge the loose-asset table into `FileReferences`, add-only.
5. Rewrite every model reference through the same controlled API.
6. Project the configured idle definition into the internal `__LingChatConfiguredIdle` group.
7. Create `Live2DModel` with the stage ticker and configured idle group.
8. Add and lay out the pending model.
9. Render one frame explicitly.
10. Only after successful rendering, replace the previous variant and report the role as Live2D-active.

Variant replacement is transactional. Keep the old model attached until the new model has loaded and rendered. A failed replacement must preserve the old instance or the static fallback.

Steps 4 and 5 are order-coupled, and both halves are load-bearing. The merge must happen **after** the fetch but **before** the rewrite, because the rewrite converts every `FileReferences` path into a `convertFileSrc` URL in place — a path injected later is never converted and the engine fetches a bare relative path. The idle projection in step 6 must come after the merge, because a discovered loose motion group (a lowercase `idle`) does not exist in `FileReferences` until the merge adds it, and `configureRuntimeIdle` throws when the configured group is missing.

`get_live2d_variant_assets` is deliberately separate from `inspect_live2d`: the latter serves the settings UI (once per dialog open, all variants), the former serves the render path (once per model load, one variant). Its result is derived from disk on every call and never persisted — `settings.yml` records only bindings, so a discovered name→file table belongs to the session, not the character file. A failure to fetch it degrades to "no injection" rather than failing the load.

## Resource Ownership

Pixi `Assets` can return shared `Texture` and `TextureSource` instances to the game stage and settings preview. Individual model instances do not own those shared textures.

When destroying a model:

```ts
model.destroy({ children: true, texture: false, baseTexture: false });
```

When destroying an application, preserve global resources. Destroying shared textures from one stage can make another stage's model disappear or produce upload errors after preview navigation.

Every model entry must also release reaction event listeners. The stage must release its ticker callback, resize observer, pointer listener, models, and Pixi application when unmounted.

## Motion Lifecycle

### Configured Idle

Cubism model3 files may put neutral, sleep, camera, and closed-eye motions in one `Idle` group. The engine's automatic idle behavior selects randomly from its configured idle group.

`configureRuntimeIdle()` copies the selected source definition into a runtime-only group containing exactly one motion. The original groups remain unchanged so emotion bindings still use their source group and index. This makes `variant.idle` authoritative without editing imported model files or adding character-specific rules.

### Reaction Completion

The engine's `motion()` option named `onFinish` is tied to sound completion in the current engine version and is not a reliable visual-motion completion callback for motions without sound.

LingChat pairs the engine's lifecycle events instead:

1. Register a filtered `motionStart` listener for the expected group, index, and FORCE priority.
2. Start the requested reaction.
3. Only after the matching `motionStart`, register `motionFinish`.
4. On finish, verify the motion manager still reports the expected group, index, and priority.
5. Remove both listeners and release the frozen gaze.
6. Let the engine's native `state.complete()` request the configured idle.

This ordering prevents an old Idle finishing during asynchronous reaction loading from consuming the reaction's finish listener. Do not replace it with motion-duration timeouts, forced parameter writes, reloads, or task scheduling. Those approaches bypass the engine state machine and fail for variable-duration or interrupted motions.

## Gaze and Eye State

There is one passive `window.pointermove` listener per mounted stage, plus the desktop-only `pet:cursor` broadcast. Both feed the same window-relative logical coordinates; the broadcast also carries the current monitor's work area in that same coordinate space.

A variant's optional `focus_anchor` is a normalized point within the model's drawable bounds, transformed through the model's current Pixi world transform, so the origin follows drawable bounds, scale, position, mode, and offsets. The local point is resolved once and cached: `getLocalBounds()` reads the live (animating) drawable vertices, so recomputing it per frame makes the origin drift with breathing and motions. When no anchor is configured the drawable-bounds center (`{x: 0.5, y: 0.5}`) is used — the same value the settings UI shows as its placeholder.

Gaze is split into direction and magnitude, and only the magnitude damps the head:

- **Direction** is the unit vector from the origin to the pointer. It drives pupil tracking, at full deflection.
- **Magnitude** `m` is `distance(origin, pointer) / distance from the origin to the work-area edge along that direction`, clamped to `0..1`. The reference distance is floored at `GAZE_REFERENCE_RATIO` (0.35) times the work area's shorter side, because a pet parked in a screen corner may have only tens of pixels of headroom in that direction, which would otherwise saturate the head immediately.

Damping is applied to the **focus controller input**, not to the parameters: `focusController.focus(ux * m, uy * m)`, so the engine's own `ParamAngle*` gains produce the damped head and no engine constants are duplicated here. The pupils are restored in the `beforeModelUpdate` handler by adding `fc * (1 / m - 1)`, which recovers the undamped spring position exactly — that spring is radial and speed-limited, so it always sits at `s * m * (ux, uy)`, and `|fc / m| <= 1` means the write is never clipped. At `m = 1` the correction is exactly zero and behaviour is byte-identical to no damping at all.

The final head angle is `breath + motion + m * focus`; only the focus share is damped, and `ParamAngleZ` scales with roughly `m²` because the engine forms it from the product `fc.x * fc.y`.

Do not read `window.screenX` or `availLeft` on the frontend for the work area. Under mixed-DPI multi-monitor setups Chromium mixes device and CSS pixels there, and this is a ratio of two distances that must come from one source — hence the Rust payload.

Read eye-open parameters from the Cubism core model. Closed eyes suspend gaze updates. A reaction freezes the current focus direction and completion restores pointer tracking. Do not infer eye state from emotion names and do not force eye parameters after a motion.

Each rig needs its own anchor. Texture margins, canvas dimensions, and drawable placement can differ between variants of the same character.

## Touch Reactions

A variant binds motions to body parts under `touch_motions`. Stroking such a part does three things: while the hand moves, the head leans with it; a bound expression goes on as soon as the stroke is accepted; and once the stroke ends and the head has settled, the motion bound to that part plays. All three parts of a binding are optional — the parts listed under `touch_motions` are exactly the touchable ones. Bindings are written by hand for now; there is no settings UI for them yet.

### Expression

A binding's `expression` names a model3 expression directly, not a LingChat emotion. It goes on at pointer-down rather than with the motion, so the face reacts as soon as the hand lands.

It is handed back once the sway has settled and no reaction is in flight — which covers both the motion case (wait for the motion) and the sway-only case (nothing to wait for). The hand-back re-applies the expression the current emotion resolves to, so it must reuse the emotion path's lookup rather than inventing its own, and it is also forced when touch mode is left, otherwise the face keeps the pat expression indefinitely.

Note this bypasses the emotion state: the touch expression is never written into the entry's emotion, precisely so the emotion path still sees a change and re-applies later.

### Sway

Sway drives the focus controller instead of writing head parameters directly. That controller maps one 2D input onto yaw, pitch, roll, and a little body lean, and forms roll from the product of the two axes, so a diagonal rub tilts the head. It is a speed- and acceleration-limited pursuit, not a spring: it lags and eases naturally but never oscillates on its own, which is why returning to centre is smooth and free.

The input is the pointer's smoothed velocity direction, scaled by speed up to a reference value, so the feel does not depend on mouse polling rate. The amplitude cap is deliberately well below full deflection: the engine multiplies it by 30 into head yaw, while the breath controller writes the same parameters with a ±15 swing. At full deflection the write saturates and the clamp removes the breathing, leaving the head stiff. Keep it around a third or less.

Standard mode otherwise holds the focus at zero so the character faces front. A stroke borrows that channel and hands it back once the sway has decayed — hence the hand-back is deferred rather than done on release, which would make the head jump.

### Closing the Eyes

A pat closes the character's eyes for two seconds, or until the hand is released, whichever comes first; a new pat re-arms it. Releasing opens them at once rather than finishing the two seconds, and only the role being touched closes its eyes.

This overrides the eye blink rather than driving it. The blink controller writes the eye-open parameters early in `internalModel.update()`, and `beforeModelUpdate` runs after it, so writing the target there wins for that frame. The write adds the delta between the current and target values rather than setting them outright, which keeps `coreModel` typed to the narrower surface the rest of the handler uses; both values sit within `0..1`, so the intermediate never leaves the parameter range and the write-time clamp cannot clip it. The override does not persist: the engine restores its saved parameters at the end of the frame, so the blink and any motion keys are unaffected on the next one.

Closing and opening are both eased over the same short duration; releasing ends the two-second window at once and the eyes ease open from wherever the lid had reached. The weight is stepped linearly against elapsed time rather than eased exponentially, because an exponential never actually reaches its target — the weight would stay non-zero forever and the per-frame hook would never go idle again.

The parameters come from the variant's `eye_blink` binding, so a rig without one simply does not close.

### Eye Tracking

Entering touch mode makes the character look at the player, meaning the pupils follow the cursor. It is scoped to the mode rather than to a stroke, so it holds from the moment the button is pressed, across a pat, and until touch mode is left; a weight fades it in and out so neither edge snaps. Every Live2D role on the stage does it, not just the one being touched.

The engine drives head and eyes from a single focus value, so the eye contribution has to be replaced rather than steered: the handler subtracts the focus value the engine wrote this frame and adds the pointer direction in its place. That cancellation is exact because nothing changes the focus between the engine's write and `beforeModelUpdate`, and both land on the same parameter — which also keeps the result inside the parameter range, so the write-time clamp cannot eat the correction. The same trick backs the pet-mode pupil compensation.

This is what keeps the sway and the gaze from fighting: while a stroke is in progress the focus holds the sway, so the head leans with the hand, and the subtraction removes exactly that share from the eyes and puts the pointer direction there instead. Pointer position is maintained by the stroke handlers themselves, because standard mode has no `pointermove` listener of its own.

Note the sway and the eyes are deliberately separated — the head follows the hand's motion, the eyes follow its position, and they only agree when the hand is moving.

### Touch Sound

A pat plays `public/audio/pet.mp3` every ~300 ms, stepping through a five-note pentatonic pattern and wrapping back rather than looping to the root, so a long stroke sounds like it is swaying rather than repeatedly dropping to the base note. Pitch comes from `playbackRate`, so one file covers the whole pattern and the timbre never changes.

Each note gets its own `Audio` element: replaying a single element would cut the previous note off, and the overlap is what makes the scale audible as a run. The step resets to the root on every pointer-down, so each pat sounds the same.

It follows the same gating as the particles — it stops when the hand leaves the model, not merely when the pointer is released — and is muted by the 气泡音量 (bubble volume) slider, which is this project's bus for short non-voice effects. The interval is driven from the same per-frame hook as everything else, so there is no separate timer to start and stop; the frame delta is already clamped, which bounds a stall to one late note instead of a burst.

### Touch Particles

A canvas overlay inside the stage's host div spawns faint pink motes and hearts along the stroke path. It is driven by distance travelled rather than by time, so a still hand emits nothing and the spacing does not change with hand speed, and it only emits while the pointer is still over a touchable part.

The canvas lives inside the host rather than beside it for two reasons: the host is a stacking context, so a `z-1` canvas beats the PIXI canvas that gets appended to the same div at runtime with no z-index; and being inside the host keeps the particles under the bubbles and touch layer, which the slot renders afterwards.

It has no resident particle pool, unlike the background effects under `standard/particles`: the animation frame loop stops as soon as the last mote dies, so an unpetted stage costs nothing. The canvas is sized lazily on spawn rather than on mount, because the host may not have been laid out yet when the component mounts.

### Gesture

A stroke is a drag, not a click, and it is measured by accumulated path length: a stroke doubles back on itself, so net displacement would not see it. A small threshold separates a deliberate rub from a drag that merely passes over the model. The motion fires on release rather than during the stroke, and only once the sway has decayed close to centre — otherwise it plays out under whatever tilt the hand left behind. That wait is a scheduling delay with a bounded backstop, not a timer used to detect the end of a motion. Pointer-up, `pointercancel`, window blur, and unmount all share one release path, because a drag can end without a pointer-up.

The motion reuses the same FORCE-priority lifecycle and `startReaction` order as emotion reactions, and does not interrupt one in flight. FORCE is required: the engine rejects a request at or below the current priority.

Regions come from geometry, not from the model's `HitAreas`. No shipped rig declares any, so the engine's hit-test always returns nothing. The eye-centred `focus_anchor` is the only cross-model hint for where the head is, so the default `head` / `body` / `legs` regions are expressed relative to it, and a variant without `focus_anchor` gets no default regions at all — a fallback anchor would put the head region on the torso. Ears, tails, and other rig-specific parts cannot be inferred and need an explicit `region`. The defaults are estimates: calibrate them against the target rig and override with `region` where they are wrong.

Standard mode bottom-anchors the drawable and clips the lower body, so the `legs` region is usually out of reach there. Do not assume it is usable.

Hit-testing converts the pointer into model-local space with `toLocal`, the inverse of the transform `updateModelFocus` applies to the gaze origin, so both stay consistent. Regions and the cached bounds are read in the same `getLocalBounds()` call as the gaze origin, because they drift with breathing and motions.

The gate is the standard-mode touch mode flag, passed to the stage as `touchEnabled`. The prop defaults to false, so the settings preview is inert without a separate switch. Only rigs with Live2D are affected; characters rendering a static avatar ignore a stroke, and desktop pet mode is not wired up.

## Layout Contracts

### Standard Mode

Standard mode uses drawable bounds for half-body composition. It scales from stage height, horizontally positions each role in scene order, locks the drawable top through the bottom-anchor transform, and allows the lower body to be clipped by the stage.

### Desktop Pet Mode

Desktop pet mode matches the static avatar contract: cover the square frame, center horizontally, and lock the drawable top to the frame top. Centering a tall portrait model vertically after cover scaling crops its head.

`scale_p`, `offset_x_p`, and `offset_y_p` remain role-controlled adjustments. Generic layout code must not contain character names, model paths, or model-specific pixel offsets.

### Settings Preview

Preview uses the same standard-mode model scale and offsets as the game view. It is a separate stage but can share global Pixi assets. Saving settings must clone raw data rather than Vue proxies.

## Import and Persistence

`import_live2d` accepts a desktop directory or ZIP and imports through a staging directory inside the character folder. It scans for model3 files, validates references and bindings, then atomically renames the staged content into `live2d/import-{nonce}`. Validation or persistence failures remove staged/promoted resources.

The optional `lingchat-live2d.json` is an import input only. After import, `settings.yml.live2d` is authoritative. Saving settings also updates the loaded backend role and the frontend store.

Inspection also reports assets the model never declared: when `FileReferences` has no `Expressions` / `Motions` section, `inspect_model` walks the model's directory for loose `.exp3.json` / `.motion3.json` files and treats each as a candidate, naming an expression after its file and giving each loose motion its own group named after its file. This is what makes a VTube Studio export selectable at all. It affects only what the settings UI may offer and what `get_live2d_variant_assets` returns — nothing discovered is written to `settings.yml`.

Do not silently persist session-only runtime state. Do not treat the import manifest as a live configuration file. The loose-asset table is exactly such session-only state: it is derived from disk on demand, so it must not be cached into the character file.

A loose-asset scan is stateful in one way that matters: the names it produces are persisted the moment a user binds one. Sorting the walk by relative path before deduplicating or uniquifying is therefore a correctness requirement, not a style choice — an unstable order would let the import-time name and the render-time name disagree, leaving the binding pointing at a group that no longer exists.

## Local Development

Install dependencies and run the frontend checks:

```bash
pnpm install
pnpm exec vue-tsc --noEmit --skipLibCheck
pnpm run build
```

For native code changes, run the normal Rust check:

```bash
cargo check --workspace
```

### Tauri Runtime Verification

Run the normal development application when a change affects Tauri commands, the asset protocol, WebView rendering, or native window behavior:

```bash
pnpm tauri dev
```

Type checking, production builds, and browser probes cannot prove those native integration paths. Use character data that you are permitted to use, and do not commit third-party model assets without redistribution rights.

## Validation Matrix

| Check               | What it proves                                              | What it does not prove                 |
| ------------------- | ----------------------------------------------------------- | -------------------------------------- |
| `vue-tsc`           | Frontend type correctness                                   | Runtime model behavior                 |
| `pnpm run build`    | Production frontend bundling                                | Native host behavior                   |
| Browser/WebGL probe | Cubism Core, engine, model assets, framebuffer rendering    | Tauri protocol and desktop pet windows |
| `pnpm tauri dev`    | Real Tauri/WebView, asset protocol, and native window modes | Installer/update behavior              |
| Installed build     | Packaging, resource sync, upgrades, normal identifier       | Source-tree development behavior       |

Before merging a change that affects lifecycle or rendering, verify at least:

- standard mode with one and multiple roles;
- desktop pet mode;
- settings preview open, save, close, and reopen;
- outfit/variant switching in both directions;
- repeated reactions and interrupted reactions;
- closed-eye motion and gaze recovery;
- route changes and stage unmount;
- invalid model fallback;
- no loss of the main model after preview use.

## Adding or Changing the Protocol

When adding a serialized field:

1. Update TypeScript types and defaults.
2. Update Rust Serde types and boundary validation.
3. Update import manifest validation and model inspection if applicable.
4. Update the settings editor and all supported locales.
5. Preserve absent-field compatibility for existing characters.
6. Keep validation aligned with the repository's current build and runtime policy; do not reintroduce removed test infrastructure.
7. Update the package tutorial and manifest example.
8. Verify the normal Tauri development application before producing an installer.

Keep fixes generic and contract-based. Character-specific calibration belongs in that character's `settings.yml`, not application source.

Loose-asset discovery does **not** go through this checklist. It adds no serialized field: `settings.yml.live2d` is unchanged, `lingchat-live2d.json` is unchanged, and existing characters parse exactly as before. The only new wire shape is `get_live2d_variant_assets`, a read-only, session-scoped IPC payload rather than a persisted contract — which is why it has its own `Live2dVariantAssets` struct instead of widening `Live2dModelInfo`.
