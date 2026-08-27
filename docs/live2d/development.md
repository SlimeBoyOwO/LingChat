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

| Path | Responsibility |
| --- | --- |
| `src/components/game/live2d/Live2DStage.vue` | Stage ownership, role synchronization, model lifecycle, runtime-result ownership, layout, expressions, motions, gaze, and lip sync integration |
| `src/components/game/live2d/live2d-stage-context.ts` | Stage-local, read-only ready/unavailable role results for avatar fallback rendering |
| `src/components/game/live2d/live2d-runtime.ts` | Cubism Core and Pixi Live2D engine loading |
| `src/components/game/standard/GameRoleAvatar.vue` | Role-intent dispatch plus shared avatar resolution, layout, animation, bubbles, touch, and effect audio |
| `src/components/game/standard/StaticRolePresentation.vue` | Traditional static image transition and load completion contract |
| `src/components/game/standard/Live2DRolePresentation.vue` | Stage-result consumption, static fallback visibility, and localized unavailable result |
| `src/components/game/live2d/model-source.ts` | Safe model3 reference rewriting and configured idle projection |
| `src/components/game/live2d/live2d-interaction.ts` | Pointer coordinate and gaze calculations |
| `src/components/game/live2d/live2d-layout.ts` | Pure layout calculations shared with tests |
| `src/components/game/live2d/live2d-motion.ts` | Motion start/finish attribution through engine lifecycle events |
| `src/components/game/live2d/useLive2dLipSync.ts` | Passive audio decoding and mouth amplitude sampling |
| `src/components/settings/character/Live2DSettings.vue` | Import, variant editing, bindings, outfit mapping, and preview |
| `src-tauri/src/api/live2d.rs` | Directory/ZIP import, inspection, validation, staging, rollback, and runtime refresh |
| `src-tauri/src/ai_service/types.rs` | Serialized `settings.yml.live2d` contract |

## Render Stack

Each mounted `GameRolesStage` creates at most one Pixi `Application`. All Live2D roles in that stage share it.

`Live2DStage` owns model readiness and unavailability results. In standard mode, role DOM is rendered through its default slot. `GameRoleAvatar` uses only the presence of `role.live2d` to choose `Live2DRolePresentation` or `StaticRolePresentation`; the Live2D presentation consumes read-only results from the nearest stage context and reuses the static presentation for fallback. `GameRolesStage` must not copy model lifecycle results into its own state or choose a role's presentation from those results. Readiness only controls when the static fallback can be hidden.

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
2. Ask Tauri for the model3 path with `get_live2d_file`.
3. Fetch model3 through Tauri's asset protocol.
4. Rewrite every model reference through the same controlled API.
5. Project the configured idle definition into the internal `__LingChatConfiguredIdle` group.
6. Create `Live2DModel` with the stage ticker and configured idle group.
7. Add and lay out the pending model.
8. Render one frame explicitly.
9. Only after successful rendering, replace the previous variant and report the role as Live2D-active.

Variant replacement is transactional. Keep the old model attached until the new model has loaded and rendered. A failed replacement must preserve the old instance or the static fallback.

## Resource Ownership

Pixi `Assets` can return shared `Texture` and `TextureSource` instances to the game stage and settings preview. Individual model instances do not own those shared textures.

When destroying a model:

```ts
model.destroy({ children: true, texture: false, baseTexture: false })
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

There is one passive `window.pointermove` listener per mounted stage. Browser coordinates are converted to Pixi stage coordinates.

If a variant has `focus_anchor`, its normalized drawable-relative point is transformed through the model's current Pixi world transform. This means the origin follows drawable bounds, scale, position, mode, and offsets. If no anchor is configured, the engine's canvas-center behavior is preserved for compatibility.

Read eye-open parameters from the Cubism core model. Closed eyes suspend gaze updates. A reaction freezes the current focus direction and completion restores pointer tracking. Do not infer eye state from emotion names and do not force eye parameters after a motion.

Each rig needs its own anchor. Texture margins, canvas dimensions, and drawable placement can differ between variants of the same character.

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

Do not silently persist session-only runtime state. Do not treat the import manifest as a live configuration file.

## Local Development

Install dependencies and run the frontend checks:

```bash
pnpm install
pnpm exec vue-tsc --noEmit --skipLibCheck
pnpm run build
```

For native code changes, run the normal Rust check:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

### Tauri Runtime Verification

Run the normal development application when a change affects Tauri commands, the asset protocol, WebView rendering, or native window behavior:

```bash
pnpm tauri dev
```

Type checking, production builds, and browser probes cannot prove those native integration paths. Use character data that you are permitted to use, and do not commit third-party model assets without redistribution rights.

## Validation Matrix

| Check | What it proves | What it does not prove |
| --- | --- | --- |
| `vue-tsc` | Frontend type correctness | Runtime model behavior |
| `pnpm run build` | Production frontend bundling | Native host behavior |
| Browser/WebGL probe | Cubism Core, engine, model assets, framebuffer rendering | Tauri protocol and desktop pet windows |
| `pnpm tauri dev` | Real Tauri/WebView, asset protocol, and native window modes | Installer/update behavior |
| Installed build | Packaging, resource sync, upgrades, normal identifier | Source-tree development behavior |

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
