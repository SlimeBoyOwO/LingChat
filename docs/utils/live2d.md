# Live2D Characters

For a complete package walkthrough, see the [Live2D character package tutorial](../live2d/authoring.md). Contributors should also read the [Live2D development guide](../live2d/development.md).

LingChat supports Live2D Cubism 3/4/5 character models as an optional visual layer. Existing PNG/WebP characters keep using the original renderer.

## Import

A complete character archive is imported from **Settings > Character > Import from archive**. Its root contains `settings.yml` and `avatar/`, and it may already include `settings.yml.live2d` configuration plus a `live2d/` resource directory. Importing that archive creates a selectable character and preserves those files.

To add Live2D to a character that already exists in the character list, open the character's settings and select the **Live2D** tab. Desktop builds accept a model directory or ZIP file; Android accepts ZIP files. LingChat copies the imported files into that character's `live2d/` directory and scans every `.model3.json` file. ZIP entry names must use `/`, not Windows-style `\`, so the imported directory structure is preserved.

The first model becomes the default variant. A variant is a visual rig inside the selected character; it does not create another selectable character. Expression and motion names are suggested from common English names and can be changed in the settings UI. Outfit names can be mapped to different model variants.

## Import Manifest

A package may include `lingchat-live2d.json`. It is used only during import; `settings.yml.live2d` is the runtime source of truth.

```json
{
  "version": 1,
  "default_variant": "default",
  "variants": {
    "default": {
      "model": "sample-rig/Sample.model3.json",
      "default_expression": "00_Default",
      "expressions": {
        "正常": "00_Default",
        "高兴": "13_Happy"
      },
      "motions": {
        "高兴": { "group": "Reactions", "index": 0, "loop": false }
      },
      "idle": { "group": "Idle", "index": 0, "loop": true },
      "eye_blink": {
        "left": "ParamEyeLOpen",
        "right": "ParamEyeROpen"
      },
      "focus_anchor": {
        "x": 0.5,
        "y": 0.25
      },
      "lip_sync": {
        "parameter": "ParamMouthOpenY",
        "gain": 1.0
      }
    }
  },
  "clothes_variants": {
    "default": "default"
  }
}
```

Model paths in the import manifest are relative to the manifest file. Runtime model paths are rewritten as character-relative paths when imported. `focus_anchor` is optional; its `x` and `y` values are normalized positions from 0 to 1 within the model's drawable bounds. When configured, mouse gaze direction originates from that point instead of the Cubism canvas center.

## Runtime Behavior

- One PixiJS application is created per mounted role stage and shared by its Live2D roles.
- Models are loaded in on-stage order and removed when their role leaves the stage.
- Cubism physics files referenced by `model3.json` are loaded by the runtime.
- LingChat emotion values select configured expressions and one-shot motions. Expression lookup uses `expressions[currentEmotion]` first and falls back to `default_expression` only when that mapping is absent. `default_expression` is not implicitly the expression for the `正常` (Normal) emotion. The configured idle motion is projected into a single runtime idle group, so the engine resumes that exact motion after a reaction instead of randomly selecting another motion from the model's source group.
- Pointer gaze uses a variant's optional drawable-relative `focus_anchor`; reactions freeze the current gaze and closed eyes suspend tracking.
- Lip sync passively decodes the existing character voice and follows the existing audio element's `currentTime`; it does not create another player or change audio routing.
- If a model fails to load, LingChat keeps the existing static avatar. A placeholder is shown only when neither visual is available.

The Live2D canvas is above static character images and below Live2D character bubbles. In a mixed scene where character images overlap, Live2D visuals appear above static visuals.

## Licensing

Cubism Core is proprietary software and is stored under `public/vendor/live2d/` with its own license and redistribution notice. It is not covered by LingChat's AGPL license. Publishing an application that imports arbitrary Live2D models may require Live2D's Expandable Application review and Publication License.

Character model files have their own copyrights and are not supplied by LingChat.
