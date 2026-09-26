# Live2D Documentation

LingChat supports Cubism 3, 4, and 5 models as an optional character visual layer. Static PNG/WebP rendering remains available and is used as the fallback when Live2D cannot load.

## Guides

| Document                                        | Audience                      | Contents                                                                                                         |
| ----------------------------------------------- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| [Character package tutorial](authoring.md)      | Character authors and testers | Package layout, import manifest, variants, expression and motion bindings, gaze calibration, and troubleshooting |
| [Development guide](development.md)             | Contributors and maintainers  | Runtime ownership, load and render lifecycle, motion tracking, layouts, resource cleanup, and validation         |
| [Live2D character overview](../utils/live2d.md) | Users                         | Import UI, manifest example, runtime behavior, and licensing                                                     |

## Important Boundaries

- A Live2D variant is a visual rig belonging to one LingChat character. It does not create a new selectable character.
- `lingchat-live2d.json` is read during import only. The imported character's `settings.yml.live2d` is the runtime source of truth.
- LingChat keeps the existing static renderer, dialogue bubbles, touch layers, audio path, and emotion protocol. Live2D is an optional visual layer, not a replacement for those systems.
- Whether a character _uses_ its Live2D rig is a per-character choice, set separately for the main stage and for desktop pet mode (`settings.yml` `avatar_mode` / `avatar_mode_p`). A character with an imported rig can still be shown as a static portrait in either place.
- Model files and Cubism Core have licenses separate from LingChat. Do not add third-party character assets to the repository without explicit redistribution rights.
