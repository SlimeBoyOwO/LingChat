"""Reduce the Flandre black MIDI to one melody and reharmonize it (requires mido).

Usage: python scripts/import-twilight-midi.py input.mid output.json
"""
import collections
import hashlib
import json
import sys
from pathlib import Path

import mido


def read_reference(source):
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
                raise ValueError("Controller and instrument events need a separate arrangement")
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


def extract_melody(score):
    ppq = score["ppq"]
    voices = [track for track in score["tracks"] if track["name"].startswith("Main Melody")]
    if not voices:
        raise ValueError("This arrangement requires the reference's Main Melody tracks")
    groups = collections.defaultdict(list)
    for bar in range(score["endTick"] // (ppq * 4) + 1):
        # Keep the lead voice for a whole bar; lower voices only fill sections where it is absent.
        for voice in voices:
            notes = [n for n in voice["notes"] if bar * ppq * 4 <= n[0] < (bar + 1) * ppq * 4]
            if notes:
                tremolo = sum(n[1] <= ppq // 4 for n in notes) >= 8 and any(n[1] >= ppq for n in notes)
                for tick, length, pitch, velocity in notes:
                    if length >= ppq // 4 and not (tremolo and length <= ppq // 4):
                        at = round(tick / (ppq / 2)) * (ppq // 2)
                        groups[at].append([at, length, pitch, velocity])
                break
    selected = []
    for at, candidates in sorted(groups.items()):
        note = max(candidates, key=lambda n: (min(n[1], ppq * 2), n[2]))
        if selected:
            previous = selected[-1]
            # A held tune above a sixteenth-note tremolo must remain a held tune.
            if (note[1] <= ppq // 4 and previous[1] >= ppq * 2
                    and at < previous[0] + previous[1] - ppq // 4 and note[2] < previous[2]):
                continue
        selected.append(note[:])
    if not selected:
        raise ValueError("No melodic notes remain after ornament reduction")
    # Choose octaves as a continuous voice, preserving small melodic intervals across
    # phrase boundaries instead of wrapping each low/high note independently.
    choices, paths, costs = [], [], {76: 0}
    previous_raw = selected[0][2]
    for index, note in enumerate(selected):
        delta = note[2] - previous_raw
        desired = delta if abs(delta) <= 7 else (delta + 6) % 12 - 6
        candidates = [pitch for pitch in range(60, 89) if pitch % 12 == note[2] % 12]
        links, next_costs = {}, {}
        for preserve_step in [True, False]:
            for pitch in candidates:
                transitions = {old: cost + abs((pitch - old) - desired) * 2
                               + max(0, abs(pitch - old) - 7) for old, cost in costs.items()
                               if not (preserve_step and index and abs(delta) <= 7) or pitch - old == delta}
                if transitions:
                    previous = min(transitions, key=transitions.get)
                    next_costs[pitch] = transitions[previous] + abs(pitch - 76) * 0.06
                    links[pitch] = previous
            if next_costs:
                break
        paths.append(links)
        costs = next_costs
        previous_raw = note[2]
    pitch = min(costs, key=costs.get)
    for links in reversed(paths):
        choices.append(pitch)
        pitch = links[pitch]
    for note, pitch in zip(selected, reversed(choices)):
        note[2] = pitch
        note[3] = max(72, min(102, note[3]))
    melody = []
    for index, note in enumerate(selected):
        next_at = selected[index + 1][0] if index + 1 < len(selected) else score["endTick"]
        note[1] = min(note[1], next_at - note[0], ppq * 8)
        if note[1] > 0:
            melody.append(note)
    return melody


def harmonize(melody, ppq, end_tick):
    # Fit half-bar triads to duration-weighted melody tones. Weak passing tones do not
    # force a new chord; a global path rewards common tones and a slower harmonic rhythm.
    qualities = [(0, 4, 7), (0, 3, 7), (0, 3, 6)]
    chords = [(root, quality, {(root + step) % 12 for step in intervals})
              for root in range(12) for quality, intervals in enumerate(qualities)]
    d_minor = {0, 2, 4, 5, 7, 9, 10}
    windows, paths, costs = [], [], [0.0] * len(chords)
    for start in range(0, end_tick, ppq * 2):
        end = min(start + ppq * 2, end_tick)
        weights = collections.Counter()
        for tick, length, pitch, _ in melody:
            overlap = max(0, min(end, tick + length) - max(start, tick))
            if overlap:
                weights[pitch % 12] += overlap / ppq * (1.3 if tick % ppq == 0 else 1)
        windows.append((start, end, bool(weights)))
        next_costs, links = [], []
        for root, quality, pcs in chords:
            fit = sum(weight * (2 if pc in pcs else -1.6) for pc, weight in weights.items())
            fit += 0.035 * len(pcs & d_minor) + (0.08 if (root, quality) == (2, 1) else 0)
            transitions = [cost - (0 if pcs == old[2] else 0.35 + 0.2 * (3 - len(pcs & old[2])))
                           for cost, old in zip(costs, chords)]
            previous = max(range(len(chords)), key=lambda i: transitions[i])
            next_costs.append(transitions[previous] + fit)
            links.append(previous)
        paths.append(links)
        costs = next_costs
    choice = max(range(len(chords)), key=lambda i: costs[i])
    chosen = []
    for links in reversed(paths):
        chosen.append(choice)
        choice = links[choice]
    harmony = []
    for (start, end, sounding), chord in zip(windows, reversed(chosen)):
        if not sounding:
            continue
        root, quality, _ = chords[chord]
        if harmony and harmony[-1][0] + harmony[-1][1] == start and harmony[-1][2:] == [root, quality]:
            harmony[-1][1] += end - start
        else:
            harmony.append([start, end - start, root, quality])
    return harmony


def convert(source):
    reference = read_reference(source)
    melody = extract_melody(reference)
    harmony = harmonize(melody, reference["ppq"], reference["endTick"])
    return {key: value for key, value in reference.items() if key != "tracks"} | {
        "arrangement": "single lead, octave reduction, independent half-bar triads",
        "melody": melody,
        "harmony": harmony,
    }


if __name__ == "__main__":
    source, destination = map(Path, sys.argv[1:])
    score = convert(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(score, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8")
    print(f"Arranged {len(score['melody'])} melody notes and {len(score['harmony'])} harmonic regions")
