"""Convert a fixed-tempo or tempo-mapped SMF into bundled score data (requires mido).

Usage: python scripts/import-twilight-midi.py input.mid output.json
"""
import collections
import hashlib
import json
import sys
from pathlib import Path

import mido


def convert(source):
    midi = mido.MidiFile(source)
    if midi.type not in (0, 1) or midi.ticks_per_beat <= 0:
        raise ValueError("Only synchronous PPQ MIDI files are supported")
    tracks, tempos, metadata = [], {0: 500000}, []
    final_tick = 0
    for index, track in enumerate(midi.tracks):
        tick, notes = 0, []
        active = collections.defaultdict(collections.deque)
        name = ""
        for event in track:
            tick += event.time
            if event.type == "set_tempo":
                tempos[tick] = event.tempo
            elif event.type in ("track_name", "copyright"):
                value = getattr(event, "name", getattr(event, "text", ""))
                value = value.encode("latin1").decode("cp932", errors="replace")
                if event.type == "track_name":
                    name = value
                if index == 0 or event.type == "copyright":
                    metadata.append([event.type, value])
            elif event.type == "note_on" and event.velocity:
                active[event.channel, event.note].append((tick, event.velocity))
            elif event.type in ("note_off", "note_on"):
                pending = active[event.channel, event.note]
                if pending:
                    start, velocity = pending.popleft()
                    notes.append([start, max(1, tick - start), event.note, velocity])
            elif event.type in ("control_change", "pitchwheel", "program_change"):
                raise ValueError("This piano-score converter does not discard controller or instrument events")
        if any(active.values()):
            raise ValueError(f"Unclosed notes in {name}")
        final_tick = max(final_tick, tick)
        if notes:
            tracks.append({"name": name, "notes": sorted(notes)})
    return {
        "source": source.name,
        "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "ppq": midi.ticks_per_beat,
        "endTick": final_tick,
        "metadata": metadata,
        "tempos": sorted(tempos.items()),
        "tracks": tracks,
    }


if __name__ == "__main__":
    source, destination = map(Path, sys.argv[1:])
    score = convert(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(score, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"Converted {sum(len(t['notes']) for t in score['tracks'])} notes in {len(score['tracks'])} tracks")
