"""Compose Gutter Butter and three bowling sounds. Python 3, standard library."""
from collections import defaultdict
from pathlib import Path
import math
import random
import struct

DRUMS = 15

def variable_length(value):
    out = [value & 127]
    value >>= 7
    while value:
        out.append(128 | (value & 127))
        value >>= 7
    return bytes(reversed(out))


class Score:
    def __init__(self, ticks_per_beat, instruments):
        self.beat = ticks_per_beat
        self.events = defaultdict(list)
        self.instruments = set()
        self.channels = len(instruments)
        for channel, program, volume, pan in instruments:
            self.instruments.add(program)
            self.control(channel, 0, program)
            self.control(channel, 3, volume)
            self.control(channel, 4, pan)
        self.control(DRUMS, 3, 82)

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
                             self.channels, 0, len(patches), 0)
        return header + table + stream


def write_wad(path, tracks):
    data = bytearray()
    directory = bytearray()
    for name, lump in tracks.items():
        directory.extend(struct.pack('<II8s', 12 + len(data), len(lump), name))
        data.extend(lump)
    path.write_bytes(struct.pack('<4sII', b'PWAD', len(tracks), 12 + len(data))
                     + data + directory)


# Drawbar organ and walking bass, with a small band passing the tune
# between lanes. Pitch classes take their usual chromatic numeric values.
ORGAN, BASS, COMP, HORN = 0, 1, 2, 3
CHORDS = [
    (41, '7'), (46, '7'), (41, '7'), (41, '7'),
    (46, '7'), (46, '7'), (41, '7'), (38, '7'),
    (43, 'm'), (36, '7'), (41, '7'), (36, '7'),
]
# Lead sheets for short answering phrases; each bar is twelve swung
# subdivisions. Negative pitches are silent. Pitch and length are literal.
RIFF = [
    [(0, 69, 1.7), (2, 72, .8), (3, 74, 2.5), (6, 72, 1.5), (8, 69, .9), (9, 67, 2.5)],
    [(0, 68, 2.5), (3, 65, .9), (5, 67, .8), (6, 70, 2.5), (9, 72, 1.7)],
    [(0, 69, 3.7), (5, 65, .85), (6, 64, .65), (7, 65, .7), (8, 67, .8), (9, 69, 2.2)],
    [(2, 72, .8), (3, 75, 2.4), (6, 72, 1.7), (8, 69, .8), (9, 65, 2.1)],
    [(0, 70, 2.4), (3, 74, 1.7), (5, 75, .8), (6, 77, 2.5), (10, 74, 1.2)],
    [(0, 72, 1.7), (2, 70, .8), (3, 68, 2.5), (6, 65, 2.5), (9, 68, 1.9)],
    [(0, 69, 2.6), (3, 72, 1.7), (5, 74, .8), (6, 77, 2.3), (9, 72, 2.5)],
    [(0, 74, 1.6), (2, 72, .7), (3, 69, 2.3), (8, 66, .9), (9, 69, 1.8)],
    [(0, 70, 3.8), (5, 74, .7), (6, 77, 1.7), (8, 79, .8), (9, 77, 2.3)],
    [(0, 76, 2.6), (3, 72, 1.7), (5, 70, .8), (6, 67, 2.4), (9, 64, 1.8)],
    [(0, 65, 2.4), (3, 69, 2.4), (6, 72, 2.3), (9, 77, 2.5)],
    [(0, 76, .8), (2, 74, .8), (3, 72, 2.4), (7, 70, .85), (9, 67, 2.3)],
]


def lane_music():
    s = Score(66, [(ORGAN, 16, 91, 57), (BASS, 32, 103, 64),
                   (COMP, 4, 69, 77), (HORN, 56, 79, 46)])
    # Two-bar entrance: an organ gliss, a rim click, a bass walk-in.
    for step, key in enumerate([55, 58, 60, 61, 62, 65, 67, 69]):
        s.note(ORGAN, step / 3, key, .29, 77+step*3)
    s.note(ORGAN, 8/3, 72, .83, 105)
    s.note(DRUMS, 3.68, 37, .1, 83)
    for i, key in enumerate([36, 43, 46, 40]):
        s.note(BASS, 4 + i, key, .8, 88+i*2)
    for tick, key in [(4, 64), (5, 67), (6, 70), (7.66, 67)]:
        s.note(COMP, tick, key, .3, 74)
    # Two blues choruses. In chorus two the trumpet plays first while the
    # organ answers a register higher, then the organ gets the last say.
    for bar in range(24):
        i = bar % 12
        start = 8 + 4*bar
        root, chord = CHORDS[i]
        next_root = CHORDS[(i+1) % 12][0]
        third = 3 if chord == 'm' else 4
        approach = next_root-1 if root <= next_root else next_root+1
        for quarter, key in enumerate([root, root+third, root+7, approach]):
            s.note(BASS, start+quarter, key, .82, [103, 77, 91, 83][quarter])
        # Rim on two and four, ride on swung eighths; short samples leave
        # room on the OPL chip for firing, lane sounds and the melody.
        for quarter in range(4):
            s.note(DRUMS, start+quarter, 42, .1, 69 if quarter % 2 else 57)
            if quarter % 2:
                s.note(DRUMS, start+quarter+.035, 37, .12, 83)
                s.note(DRUMS, start+quarter+2/3, 42, .1, 48)
        s.note(DRUMS, start, 36, .16, 88)
        if i in (3, 7, 11):
            s.note(DRUMS, start+3+2/3, 45, .16, 65)
        for accent in (1+2/3, 3):
            s.note(COMP, start+accent, root+12+third, .25, 66)
            s.note(COMP, start+accent+.02, root+22, .28, 72)
        lead = HORN if bar in range(12, 20) else ORGAN
        for offset, key, duration in RIFF[i]:
            # Give the second chorus its own contour and breathing spaces.
            if 12 <= bar < 16 and offset >= 7:
                s.note(ORGAN, start+offset/3, key+12, duration/3, 63)
            elif bar >= 20 and offset < 6:
                s.note(ORGAN, start+offset/3, key+12, duration/3, 78)
            else:
                s.note(lead, start+offset/3, key, duration/3, 96 if offset==0 else 86)
        if i in (2, 6, 10) and bar < 12:
            s.note(HORN, start+3+2/3, root+19, .25, 62)
    # Pickup into the opening organ gliss.
    return s.mus(104)


def effect(duration, sample_function, seed):
    rate = 11025
    rng = random.Random(seed)
    data = bytearray()
    for i in range(round(rate*duration)):
        t = i / rate
        taper = min(1, i / 12, (duration-t) * 60)
        data.append(round(128 + 122 * max(-1, min(1, taper * sample_function(t,rng)))))
    return struct.pack('<HHI', 3, rate, len(data)+32) + bytes([128])*16 + data + bytes([128])*16


def roll():
    noise_lp = 0
    def sample(t, rng):
        nonlocal noise_lp
        noise_lp = .81 * noise_lp + .19 * rng.uniform(-1,1)
        e = (1-math.exp(-55*t))*math.exp(-2.3*t)
        # Spin slows as it rolls; the off-centre core bumps the wood.
        phase = 2*math.pi*(14*t - 3.5*t*t)
        bumps = (0.5 + 0.5*math.cos(phase))**6
        bass = math.sin(2*math.pi*(71*t - 5*t*t))
        body = math.sin(2*math.pi*(113*t - 8*t*t))
        return e * (.24*bass + .17*body + (.48+bumps*.66)*noise_lp)
    return effect(.72, sample, 300)


def clatter():
    strikes = [(0, 430, .63), (.031, 590, .42), (.073, 499, .51),
               (.132, 655, .33), (.189, 560, .26), (.270, 730, .17)]
    def sample(t, rng):
        value=0
        for start, freq, gain in strikes:
            u=t-start
            if u<0: continue
            tone = math.sin(2*math.pi*freq*u)*math.exp(-43*u)
            overtone = math.sin(2*math.pi*freq*2.67*u)*math.exp(-83*u)
            crack = rng.uniform(-1,1)*math.exp(-190*u)
            value += gain*(.54*tone+.31*overtone+.44*crack)
        return value
    return effect(.42, sample, 101)


def strike():
    # Four dry scoreboard chimes and a low mechanical relay under the first.
    chimes=[(0,523.25),(.090,659.26),(.182,783.99),(.280,1046.50)]
    def sample(t, rng):
        v=.18*rng.uniform(-1,1)*math.exp(-68*t)
        for start,freq in chimes:
            u=t-start
            if u<0:continue
            v+=.42*min(1,950*u)*math.exp(-9*u)*(
                math.sin(2*math.pi*freq*u)+.24*math.sin(2*math.pi*freq*2.5*u))
        return v
    return effect(.86, sample, 10)


if __name__ == '__main__':
    write_wad(Path(__file__).with_name('bowl-music.wad'),
              {b'D_E1M9': lane_music(), b'DSBRROLL': roll(),
               b'DSBRPINS': clatter(), b'DSBRSTRK': strike()})
