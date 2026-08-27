# Live2D Character Package Tutorial

This tutorial creates a Live2D add-on package for an existing LingChat character. Desktop builds can import a directory or ZIP from that character's **Live2D** settings tab; Android imports ZIP files.

This is different from a complete character archive. A character archive has `settings.yml`, `avatar/`, and optionally `live2d/` at its root and is imported from **Settings > Character > Import from archive**. It creates a selectable character and preserves an included `settings.yml.live2d` configuration and `live2d/` resource directory. A Live2D add-on package only adds visual rigs to a character that already exists in the character list.

## 1. Check the Export

Use a Cubism 3, 4, or 5 Web export containing a `.model3.json` file. The model file must reference its resources with relative paths.

A typical package looks like this:

```text
my-character-live2d/
├── lingchat-live2d.json
├── casual/
│   ├── Casual.model3.json
│   ├── Casual.moc3
│   ├── Casual.physics3.json
│   ├── expressions/
│   ├── motions/
│   └── textures/
└── uniform/
    ├── Uniform.model3.json
    ├── Uniform.moc3
    ├── Uniform.physics3.json
    ├── expressions/
    ├── motions/
    └── textures/
```

LingChat validates referenced files and rejects absolute paths, URL references, missing files, and paths that escape the imported package.

## 2. Inspect Expressions and Motions

Open each `.model3.json` in a JSON-aware editor. Bindings use names and array indexes from `FileReferences`.

```json
{
  "FileReferences": {
    "Expressions": [
      { "Name": "Default", "File": "expressions/default.exp3.json" },
      { "Name": "Happy", "File": "expressions/happy.exp3.json" }
    ],
    "Motions": {
      "Idle": [
        { "File": "motions/idle.motion3.json" },
        { "File": "motions/sleep.motion3.json" }
      ],
      "Reactions": [
        { "File": "motions/wave.motion3.json" },
        { "File": "motions/angry.motion3.json" }
      ]
    }
  }
}
```

The first reaction above is `Reactions[0]`; the second is `Reactions[1]`. Indexes are zero-based.

Do not assume every motion in the source `Idle` group is suitable for automatic idle playback. Sleep, camera, and closed-eye motions are often stored in the same group. Select one intentional idle in the manifest. LingChat projects that selection into a one-motion runtime group so the engine resumes the configured idle after reactions.

## 3. Add the Import Manifest

Place `lingchat-live2d.json` at the package root:

```json
{
  "version": 1,
  "default_variant": "casual",
  "variants": {
    "casual": {
      "model": "casual/Casual.model3.json",
      "default_expression": "Default",
      "expressions": {
        "正常": "Default",
        "高兴": "Happy"
      },
      "motions": {
        "高兴": { "group": "Reactions", "index": 0, "loop": false }
      },
      "idle": { "group": "Idle", "index": 0, "loop": true },
      "eye_blink": {
        "left": "ParamEyeLOpen",
        "right": "ParamEyeROpen"
      },
      "focus_anchor": { "x": 0.5, "y": 0.22 },
      "lip_sync": {
        "parameter": "ParamMouthOpenY",
        "gain": 1.0
      }
    },
    "uniform": {
      "model": "uniform/Uniform.model3.json",
      "default_expression": "Default",
      "expressions": {
        "正常": "Default",
        "高兴": "Happy"
      },
      "motions": {
        "高兴": { "group": "Reactions", "index": 0, "loop": false }
      },
      "idle": { "group": "Idle", "index": 0, "loop": true },
      "eye_blink": {
        "left": "ParamEyeLOpen",
        "right": "ParamEyeROpen"
      },
      "focus_anchor": { "x": 0.5, "y": 0.28 },
      "lip_sync": {
        "parameter": "ParamMouthOpenY",
        "gain": 1.0
      }
    }
  },
  "clothes_variants": {
    "default": "casual",
    "制服": "uniform"
  }
}
```

### Manifest Fields

| Field | Meaning |
| --- | --- |
| `version` | Manifest protocol version. Currently `1`. |
| `default_variant` | Variant used when no outfit mapping applies. It must exist in `variants`. |
| `variants` | Named Live2D rigs belonging to this character. |
| `model` | Path to the variant's `.model3.json`, relative to the manifest. |
| `default_expression` | Fallback expression used only when the current LingChat emotion has no entry in `expressions`. It does not define the expression for the `正常` (Normal) emotion; map that emotion explicitly when needed. |
| `expressions` | LingChat emotion name to model3 expression `Name`. |
| `motions` | LingChat emotion name to model3 motion group and zero-based index. |
| `idle` | The exact motion used for automatic idle playback. |
| `eye_blink` | Cubism parameter IDs used to detect whether the eyes are open. |
| `focus_anchor` | Optional gaze origin within drawable bounds; both values are in `0..1`. |
| `lip_sync` | Mouth-open parameter and optional amplitude gain. |
| `clothes_variants` | LingChat outfit name to variant name. Use `default` for the default outfit. |

If no manifest is included, LingChat scans all `.model3.json` files, creates variants, and suggests common expression and motion bindings. Review those suggestions in settings; keyword matching cannot understand the artistic intent of every motion.

## 4. Calibrate Each Variant

Import the package, open the character's **Live2D** settings, and select each variant separately.

1. Choose the fallback expression to use when an emotion has no explicit expression mapping. This is not automatically the `正常` (Normal) expression.
2. Bind each LingChat emotion, including `正常` when required, to an expression and optional one-shot motion.
3. Select an idle that keeps the character in the expected neutral state.
4. Confirm `ParamEyeLOpen` and `ParamEyeROpen`, or enter the model's actual eye-open parameter IDs.
5. Confirm the mouth parameter, usually `ParamMouthOpenY`.
6. Adjust the gaze anchor to the center between the rendered eyes.
7. Map every LingChat outfit to the correct variant.
8. Save, leave settings, and test the character in both standard and desktop pet modes.

`focus_anchor` is relative to each variant's drawable bounds, not its texture and not another variant's canvas. Two rigs of the same character can have different transparent margins and head positions. Measure and save each variant independently; copying one rig's values to another can place the gaze origin on the torso.

## 5. Import and Verify

For an existing character:

1. Open **Settings > Character** and select the character.
2. Open the **Live2D** tab.
3. On desktop, select a directory or ZIP. On Android, select a ZIP.
4. Review every discovered variant and binding.
5. Save the character settings.
6. Enter a standard conversation and trigger several configured emotions.
7. Switch outfits and verify that the model, expression, idle, and gaze anchor all change together.
8. Enter desktop pet mode and confirm that the head is visible and top-aligned in the circular frame.

For a new selectable character, import a complete character archive first. If that archive does not already include Live2D configuration and resources, open the imported character's **Live2D** settings tab and import this add-on package separately. A failed add-on import does not remove the existing character.

## Troubleshooting

### The model does not appear

- Confirm the `.model3.json`, `.moc3`, textures, and referenced physics/expression/motion files are all inside the package.
- Check that model3 references are relative and use the correct case.
- Keep a valid static avatar configured; LingChat intentionally shows it when Live2D loading or first rendering fails.

### A reaction ends with closed eyes or sleep

Check the configured `idle` group and index. The source model may contain sleep or camera motions in its `Idle` group. LingChat resumes the configured entry, so an incorrect index remains an incorrect artistic choice.

### Gaze starts from the torso

Set `focus_anchor` for the active variant. The fallback is the model's Cubism canvas center, which may not be near the eyes. Calibrate every variant separately.

### Outfit switching keeps the wrong rig

Check that the exact LingChat outfit name exists in `clothes_variants` and that its target exists in `variants`. The default outfit uses the key `default`.

### Import fails after extraction

LingChat imports through a staging directory and removes staged or promoted resources on validation failure. Correct the reported missing file, invalid binding, path containment, or manifest error and import again.

## Packaging and Licensing

ZIP the package contents so that the manifest and model directories retain their relative layout. ZIP entry names must use the standard `/` separator; do not store Windows-style `\` separators, because the archive safety layer sanitizes them as filename characters and the directory layout will be lost.

Model copyrights and Cubism licensing are separate from LingChat's AGPL license. Distribute only assets for which you have permission.
