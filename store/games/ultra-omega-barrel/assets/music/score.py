"""Compose the Ultra Omega Barrel title and boss music. Run with Python 3."""
from collections import defaultdict
from pathlib import Path
import math
import random
import struct


BASS, BRASS, BELL, STRINGS, DRUMS = 0, 1, 2, 3, 15
# D, Eb, G, Bb, A, C; literal pitch numbers follow General MIDI numbering.
D, EB, G, BB, A, C = 38, 39, 43, 34, 33, 36
# Each cell is a half beat. None is a rest. The two material families answer
# one another: the iron theme drops a fifth, while the inner doll climbs.
IRON = [
    [(0, 62, 1.25), (1.5, 65, .45), (2, 63, .42), (2.5, 57, .42)],
    [(0, 58, .85), (1, 63, .43), (1.5, 62, 1.25)],
    [(.5, 67, .45), (1, 65, .85), (2, 62, .4), (2.5, 60, .4)],
    [(0, 57, 1.7), (2, 61, .9)],
]
DOLL = [
    [(0, 74, .55), (1, 77, .38), (1.5, 75, .36), (2.5, 69, .37)],
    [(.5, 70, .4), (1, 75, .4), (2, 74, .78)],
    [(0, 67, .4), (.5, 70, .4), (1, 74, .85), (2.5, 77, .4)],
    [(0, 76, .8), (1, 73, .4), (1.5, 69, .38), (2, 73, .7)],
]
COUNTER = [
    [(0, 53, 1.45), (2, 57, .65)], [(0, 55, 2.6)],
    [(0, 58, 1.6), (2, 60, .7)], [(0, 61, 1.9), (2.5, 57, .35)],
]


def variable_length(value):
    out = [value & 127]
    value >>= 7
    while value:
        out.append(128 | (value & 127))
        value >>= 7
    return bytes(reversed(out))


class Score:
    def __init__(self, ticks_per_beat):
        self.beat = ticks_per_beat
        self.events = defaultdict(list)
        self.instruments = set()
        for channel, program, volume, pan in [
            (BASS, 38, 92, 61), (BRASS, 61, 85, 48),
            (BELL, 10, 80, 83), (STRINGS, 48, 67, 69),
        ]:
            self.instruments.add(program)
            self.control(channel, 0, program)
            self.control(channel, 3, volume)
            self.control(channel, 4, pan)
        self.control(DRUMS, 3, 83)

    def control(self, channel, control, value):
        self.events[0].append((0, bytes([0x40 | channel, control, value])))

    def note(self, channel, beat, pitch, length, velocity):
        start, end = round(beat * self.beat), round((beat + length) * self.beat)
        self.events[start].append((2, bytes([0x10 | channel, 128 | pitch, velocity])))
        self.events[end].append((1, bytes([channel, pitch])))
        if channel == DRUMS:
            self.instruments.add(pitch + 100)

    def phrase(self, channel, base, phrase, velocity, transpose=0):
        for offset, key, length in phrase:
            self.note(channel, base + offset, key + transpose, length, velocity)

    def mus(self, end_beat):
        end_tick = round(end_beat * self.beat)
        if max(self.events) >= end_tick:
            raise ValueError("A note overlaps the end of the score")
        times = sorted(self.events) + [end_tick]
        stream = bytearray()
        for tick, next_tick in zip(times, times[1:]):
            events = sorted(self.events[tick])
            for index, (_, data) in enumerate(events):
                stream.extend(bytes([data[0] | (128 if index == len(events)-1 else 0)]) + data[1:])
            stream.extend(variable_length(next_tick - tick))
        stream.append(0x60)
        patches = sorted(self.instruments)
        table = struct.pack('<' + 'H' * len(patches), *patches)
        header = struct.pack('<4s6H', b'MUS\x1a', len(stream), 16 + len(table),
                             4, 0, len(patches), 0)
        return header + table + stream


def boss_music():
    s = Score(58)
    roots = [D, EB, BB, A, D, G, EB, A]
    for measure in range(48):
        start = measure * 3
        cell = measure % 4
        root = roots[measure % 8]
        bridge = 24 <= measure < 32
        final = measure >= 40
        # Foundation. The low note and deliberately late rivet describe a
        # three-stroke press. Leave the bridge almost empty for the tiny voice.
        if bridge:
            root = [D, EB, G, A][cell]
            s.note(BASS, start, root, 1.65, 63)
            s.note(STRINGS, start + .08, root + 19, 2.60, 62)
        else:
            riff = [(0, 0, .67, 104), (1, 12, .25, 72),
                    (1.5, 7, .4, 87), (2.5, 0, .36, 78)]
            if cell == 3:
                riff = [(0, 0, .6, 107), (1, 7, .55, 85), (2, 12, .32, 81),
                        (2.5, 13 if measure % 8 == 3 else 11, .35, 74)]
            for offset, delta, length, strength in riff:
                s.note(BASS, start+offset, root+delta, length, strength)
            for offset, pitch, strength in [(0, 36, 102), (1, 37, 76),
                                           (1.53, 42, 52), (2, 38, 88)]:
                s.note(DRUMS, start+offset, pitch, .13, strength)
            if cell in (1, 3):
                s.note(DRUMS, start+2.52, 56, .17, 67)
            if measure in (7, 15, 31, 39, 46):
                for offset, pitch in [(1.75, 47), (2.25, 45), (2.75, 41)]:
                    s.note(DRUMS, start+offset, pitch, .15, 85)
            if measure >= 16 and measure % 4 == 0:
                s.note(DRUMS, start, 49, .48, 78)
        if measure < 8:
            if measure % 2 == 0:
                s.note(BRASS, start, root + 12, .75, 87)
                s.note(BRASS, start+.04, root + 19, .67, 77)
            if measure >= 4:
                s.phrase(BELL, start, DOLL[cell], 68, -12)
        elif measure < 24:
            s.phrase(BRASS, start, IRON[cell], 90 if measure < 16 else 100)
            if measure >= 16:
                s.phrase(BELL, start, DOLL[cell], 76)
            if cell in (1, 3):
                s.note(STRINGS, start, root+24, 2.65, 70)
        elif bridge:
            # Each pair of measures opens a smaller doll. Their identical
            # contour gains another octave before slipping back into the march.
            s.phrase(BELL, start, DOLL[cell], 83 if cell < 2 else 71,
                     -12 if measure < 28 else 0)
            s.note(DRUMS, start+2, 54, .12, 57)
        elif not final:
            s.phrase(BRASS, start, COUNTER[cell], 101)
            s.phrase(BELL, start, DOLL[cell], 84)
        elif measure != 47:
            s.phrase(BRASS, start, IRON[cell], 104)
            s.phrase(BELL, start, DOLL[cell], 88)
            s.phrase(STRINGS, start, COUNTER[cell], 71)
        else:
            s.note(BRASS, start, 57, 1.45, 100)
            for offset, pitch in [(1.5, 69), (2, 73), (2.5, 75)]:
                s.note(BELL, start+offset, pitch, .32, 75)
    return s.mus(48 * 3)


def title_music():
    s = Score(65)
    for bar, root in enumerate([D, EB, A, D, BB, G, A, D]):
        t = bar * 3
        s.note(BASS, t, root, 1.65 if bar in (3, 7) else .82, 103)
        s.note(DRUMS, t, 41, .32, 100)
        if bar not in (3, 7):
            s.note(BASS, t+2, root+7, .64, 80)
            s.note(DRUMS, t+2, 56, .18, 69)
        if bar < 3 or bar in (4, 5, 6):
            s.phrase(BRASS, t, IRON[bar % 4], 93)
            s.phrase(BELL, t, DOLL[bar % 4], 72)
        else:
            s.note(BRASS, t, 62, 1.95, 98)
            s.note(STRINGS, t, 53, 1.95, 74)
            s.note(BELL, t+.5, 81, .4, 65)
            s.note(BELL, t+1, 74, 1.2, 75)
    return s.mus(8 * 3)



def split_sound():
    """Synthesize a can seam popping, then two successively smaller shells."""
    rate = 11025
    rng = random.Random(0xCA41)
    samples = bytearray()
    last_noise = 0.0
    for index in range(round(.54 * rate)):
        t = index / rate
        noise = rng.uniform(-1, 1)
        # The pressure release gives way to a short metallic ring.
        value = .19 * (noise - .6 * last_noise) * math.exp(-t * 64)
        last_noise = noise
        value += .33 * math.sin(2 * math.pi * (92*t + 2.6*(1-math.exp(-34*t)))) * math.exp(-27*t)
        for onset, frequency, gain in [(0, 310, .22), (.066, 475, .16), (.129, 690, .115)]:
            u = t - onset
            if u < 0:
                continue
            attack = min(1, u * 820)
            for ratio, decay, weight in [(1, 24, 1), (2.37, 48, .42), (3.92, 67, .25)]:
                ring = math.sin(2 * math.pi * frequency * ratio * u)
                value += gain * weight * attack * math.exp(-decay*u) * ring
        value *= min(1, index / 6)
        samples.append(max(0, min(255, round(128 + 124 * value))))
    padding = bytes([128]) * 16
    return struct.pack('<HHI', 3, rate, len(samples) + 32) + padding + samples + padding


def write_wad(path, tracks):
    data = bytearray()
    directory = bytearray()
    for name, lump in tracks.items():
        directory.extend(struct.pack('<II8s', 12 + len(data), len(lump), name))
        data.extend(lump)
    path.write_bytes(struct.pack('<4sII', b'PWAD', len(tracks), 12 + len(data))
                     + data + directory)


if __name__ == '__main__':
    theme = title_music()
    write_wad(Path(__file__).with_name('foundry-score.wad'),
              {b'D_E1M8': boss_music(), b'D_INTRO': theme, b'D_INTROA': theme,
               b'DSSHELL': split_sound()})
