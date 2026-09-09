# Script terminal entry errors

A story can deny subsequent formal starts after a persisted boolean becomes true:

```yaml
script_settings:
  persistent_vars: [act4_done]
  entry_error:
    variable: act4_done
    message: |
      SCRIPT_CORRUPTED
      Reset this story's memory to restart.
```

Set `act4_done = true` through an ordinary `set_variable` event at the ending. Normal script completion persists it. The menu checks before entry effects; `start_script` checks again before reserving a run or switching characters. Only the exact boolean `true` from that script owner's saved state triggers the gate. Undeclared variables and absent settings do not block stories. The message is bounded to 512 plain-text characters; control characters other than newline and tab are rejected.

The entry query is read-only and does not increment playthroughs. The existing memory reset/uninstall transaction removes the owner's state and clears the gate. Preview remains isolated from real saved state. No chapter or character asset is deleted to create this effect. Older engines can retain an intro routing fallback to an error chapter.

Force-choice cursor warps now stay inside the visible choices panel, intersected with the viewport. This is periodic pull-back during the existing five-second effect, not an OS cursor clip. Escape, focus loss, hidden document, submission, event replacement and component unmount still cancel the warp ticket. Non-forced buttons stay disabled, and the engine still validates the forced submission.
