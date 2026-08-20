# Live2D Characters

LingChat supports Live2D Cubism 3/4/5 character models as an optional visual layer. Existing PNG/WebP characters keep using the original renderer.

## Import

Open a character's settings and select the **Live2D** tab. Desktop builds accept a model directory or ZIP file; Android accepts ZIP files. LingChat copies the imported files into the character's `live2d/` directory and scans every `.model3.json` file.

The first model becomes the default variant. A variant is a visual rig inside the selected character; it does not create another selectable character. To make a model such as Nori appear as its own character, create that character first and import the model into its Live2D tab. Expression and motion names are suggested from common English names and can be changed in the settings UI. Outfit names can be mapped to different model variants.

Character creation also accepts an optional Live2D source. The normal character is created first, then the Live2D files are imported. Existing static character creation behavior is unchanged.

## Import Manifest

A package may include `lingchat-live2d.json`. It is used only during import; `settings.yml.live2d` is the runtime source of truth.

```json
{
  "version": 1,
  "default_variant": "default",
  "variants": {
    "default": {
      "model": "Nori/Nori.model3.json",
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
- LingChat emotion values select configured expressions and one-shot motions. Idle resumes after a reaction motion.
- Pointer gaze uses a variant's optional drawable-relative `focus_anchor`; reactions freeze the current gaze and closed eyes suspend tracking.
- Lip sync passively decodes the existing character voice and follows the existing audio element's `currentTime`; it does not create another player or change audio routing.
- If a model fails to load, LingChat keeps the existing static avatar. A placeholder is shown only when neither visual is available.

The Live2D canvas is above static character images and below Live2D character bubbles. In a mixed scene where character images overlap, Live2D visuals appear above static visuals.

## Development Host

Live2D frontend changes can be tested without rebuilding an installer. Run the existing **Build Development (Full Test)** workflow with platform `windows-dev-host`, download `lingchat-windows-dev-host`, and start it with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/start-windows-dev-host.ps1 `
  -ExePath C:\path\to\ling_chat.exe
```

The launcher creates isolated data under `.dev-data/live2d`, isolates the Tauri settings store, starts Vite on port 1420, and then starts the precompiled Rust host. Vue, TypeScript, and CSS changes use Vite HMR. Rust command changes still require rebuilding the Dev Host.

Set `LINGCHAT_DATA_DIR` to explicitly select a data directory when running a desktop debug host.

## Licensing

Cubism Core is proprietary software and is stored under `public/vendor/live2d/` with its own license and redistribution notice. It is not covered by LingChat's AGPL license. Publishing an application that imports arbitrary Live2D models may require Live2D's Expandable Application review and Publication License.

Character model files have their own copyrights and are not supplied by LingChat.
