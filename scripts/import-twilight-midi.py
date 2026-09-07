"""Import DMBN's normal U.N. Owen piano score into separate game voices (requires mido).

Usage: python scripts/import-twilight-midi.py un-normal.mid output.json
"""
import collections
import hashlib
import json
import sys
from pathlib import Path

import mido


def convert(source):
    midi = mido.MidiFile(source)
    if midi.type != 1 or midi.ticks_per_beat <= 0 or len(midi.tracks) != 2:
        raise ValueError("Expected DMBN's two-track Normal piano arrangement")
    tracks, tempos, meters = [], {0: 500000}, {0: [4, 4]}
    end_tick = 0
    for track in midi.tracks:
        tick, notes = 0, []
        active = collections.defaultdict(collections.deque)
        for event in track:
            tick += event.time
            if event.type == "set_tempo":
                tempos[tick] = event.tempo
            elif event.type == "time_signature":
                meters[tick] = [event.numerator, event.denominator]
            elif event.type == "note_on" and event.velocity:
                active[event.channel, event.note].append((tick, event.velocity))
            elif event.type in ("note_off", "note_on"):
                if not active[event.channel, event.note]:
                    raise ValueError("Unmatched note-off")
                start, velocity = active[event.channel, event.note].popleft()
                notes.append([start, tick - start, event.note, velocity])
            elif event.type == "control_change":
                # Replace pedal with the synth's bounded release to keep the high lead distinct.
                if event.control != 64:
                    raise ValueError("Unsupported controller in the reference")
            elif not event.is_meta:
                raise ValueError(f"Unsupported MIDI event: {event.type}")
        if any(active.values()):
            raise ValueError("Unclosed notes")
        if not notes:
            raise ValueError("Both piano hands must contain notes")
        tracks.append(sorted(notes))
        end_tick = max(end_tick, tick)

    start_tick = min(note[0] for track in tracks for note in track)
    groups = collections.defaultdict(list)
    for note in tracks[0]:
        groups[note[0]].append(note)
    melody, harmony = [], []
    for notes in groups.values():
        notes.sort(key=lambda n: n[2], reverse=True)
        tick, length, pitch, velocity = notes[0]
        melody.append([tick - start_tick, length, pitch, velocity])
        for tick, length, pitch, velocity in notes[1:]:
            harmony.append([tick - start_tick, length, pitch, velocity])
    for current, following in zip(melody, melody[1:]):
        current[1] = min(current[1], following[0] - current[0])
    bass = [[tick - start_tick, length, pitch, velocity] for tick, length, pitch, velocity in tracks[1]]

    def timeline(events):
        initial = events[max(tick for tick in events if tick <= start_tick)]
        return [[0, initial]] + [[tick - start_tick, value] for tick, value in sorted(events.items()) if tick > start_tick]

    return {
        "source": "U.N. Owen was her？_Normal.mid",
        "sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "composer": "ZUN / 上海アリス幻樂団",
        "arranger": "DMBN / 東方ピアノEasyモード",
        "sourceUrl": "https://easypianoscore.jp/sheetList.php?titleid=kouma",
        "downloadUrl": "https://easypianoscore.jp/download.php?musicName=un&musicLevel=normal&ext=zip&inst=",
        "termsUrl": "https://easypianoscore.jp/kenri.html",
        "arrangement": "Normal piano reference; original pitches in all voices; bounded release instead of pedal",
        "ppq": midi.ticks_per_beat,
        "trimmedTicks": start_tick,
        "endTick": end_tick - start_tick,
        "tempos": timeline(tempos),
        "meters": [[tick, *meter] for tick, meter in timeline(meters)],
        "melody": melody,
        "harmony": sorted(harmony),
        "bass": bass,
    }


if __name__ == "__main__":
    source, destination = map(Path, sys.argv[1:])
    score = convert(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(score, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"Imported {len(score['melody'])} melody, {len(score['harmony'])} harmony and {len(score['bass'])} bass notes")
