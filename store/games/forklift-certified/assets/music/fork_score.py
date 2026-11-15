"""Compose Graveyard Gearbox and four forklift sounds. Python 3, standard library."""
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


def effect(duration, sample_function, seed):
    rate = 11025
    rng = random.Random(seed)
    data = bytearray()
    for i in range(round(rate*duration)):
        t = i / rate
        taper = min(1, i / 12, (duration-t) * 60)
        data.append(round(128 + 122 * max(-1, min(1, taper * sample_function(t,rng)))))
    return struct.pack('<HHI', 3, rate, len(data)+32) + bytes([128])*16 + data + bytes([128])*16


BASS, GUITAR, REED, MUTED = range(4)
# E is the pedal; the chords shift under it in half-steps like power gears.
# The rhythm guitar hooks all sit below the forklift warning's register.
ROOTS = (40, 40, 43, 45, 40, 46, 45, 35)
HOOKS = [
    [(0, 0, .62), (.75, 0, .19), (1.5, 3, .38),
     (2, 5, .63), (2.75, 6, .18), (3, 5, .66)],
    [(.25, 0, .42), (1, -2, .43), (1.75, 0, .18),
     (2.25, 3, .68), (3.5, 0, .36)],
    [(0, 0, .82), (1.25, 7, .34), (1.75, 5, .16),
     (2, 3, .84), (3.25, 0, .46)],
    [(0, 0, .61), (.75, 5, .23), (1.25, 3, .34),
     (2, 0, .62), (2.75, -1, .19), (3.5, 0, .34)],
]
# Harmonica takes the mic for the middle eight. The guitar answers on
# offbeats, leaving a hole where the live vehicle warning cuts through.
RESPONSE = [
    [(0, 64, .95), (1.25, 67, .46), (2, 70, .83), (3.25, 69, .56)],
    [(.5, 67, .32), (1, 64, 1.58), (3, 62, .72)],
    [(0, 67, 1.28), (1.5, 70, .42), (2.5, 69, .38), (3, 67, .66)],
    [(.25, 64, 1.50), (2, 62, .38), (2.75, 59, .44), (3.5, 62, .32)],
]


def music():
    s=Score(66,[(BASS,34,100,64),(GUITAR,29,91,50),
                (REED,22,79,80),(MUTED,28,75,76)])
    for bar in range(32):
        t=4*bar
        root=ROOTS[bar%8]
        sparse=16<=bar<20
        # The pickup stays put while the rest of the band shifts; the
        # choked answering chords give the FM guitar a ragged garage sound.
        for i,(offset,interval,length) in enumerate(HOOKS[bar%4]):
            if sparse and i%2: continue
            s.note(BASS,t+offset,root-12+interval,length,107 if i==0 else 83)
            if not sparse and (i in (0,3) or bar>=24):
                s.note(GUITAR,t+offset,root+interval,length*.89,98 if i==0 else 85)
                s.note(GUITAR,t+offset+.015,root+interval+7,length*.87,73)
        if 8<=bar<16 or bar>=24:
            for offset in (1.25,3.25):
                s.note(MUTED,t+offset,root+12,.15,85)
                s.note(MUTED,t+offset+.02,root+19,.16,63)
        if 16<=bar<24:
            for offset,key,length in RESPONSE[bar%4]:
                s.note(REED,t+offset,key,length,92)
            if sparse and bar%2:
                s.note(GUITAR,t+3,root+7,.31,83)
        elif bar%4==3 and bar>3:
            for offset,key in [(1.75,64),(2.5,67),(3,62)]:
                s.note(REED,t+offset,key,.30 if key!=62 else .64,73)
        for b in range(4):
            if not sparse or b in (1,3):
                s.note(DRUMS,t+b+.5,42,.11,55 if b%2 else 66)
            if b in (0,2):
                s.note(DRUMS,t+b,36,.12,103 if b==0 else 83)
            else:
                s.note(DRUMS,t+b+.035,38 if not sparse else 37,.12,95)
        if bar%4 in (0,2) and not sparse:
            s.note(DRUMS,t+2.75,36,.13,81)
        if bar%8==7:
            for offset,key in [(2.25,48),(2.65,47),(3.4,43),(3.74,41)]:
                s.note(DRUMS,t+offset,key,.16,81)
        elif bar in (8,16,24,28):
            s.note(DRUMS,t,49,.45,72)
    return s.mus(128)


def beep():
    # A single pulse. The live driver sets the three-pulse warning cadence.
    def sample(t,rng):
        attack=min(1,t*225)
        gate=min(1,max(0,(.166-t)*100))
        phase=2*math.pi*(796*t+.085*math.sin(2*math.pi*53*t))
        ring=math.sin(phase)+.23*math.sin(3*phase)
        return .63*attack*gate*ring
    return effect(.18,sample,735)


def hydraulics():
    lp=0
    def sample(t,rng):
        nonlocal lp
        white=rng.uniform(-1,1)
        lp=.89*lp+.11*white
        hiss=(white-lp)*.26*(1-math.exp(-47*t))*math.exp(-6*t)
        piston=.21*math.sin(2*math.pi*(220*t-133*t*t))*math.exp(-6*t)
        hit=0
        for onset,hz,gain in [(.22,167,.57),(.247,340,.35),(.344,520,.18)]:
            u=t-onset
            if u>=0:
                hit+=gain*min(1,750*u)*math.exp(-27*u)*(
                    .65*math.sin(2*math.pi*hz*u)+.30*math.sin(2*math.pi*hz*2.64*u))
        return hiss+piston+hit
    return effect(.52,sample,37)


def cargo_smash():
    lp=0
    def sample(t,rng):
        nonlocal lp
        lp=.68*lp+.32*rng.uniform(-1,1)
        thump=.59*math.exp(-17*t)*math.sin(2*math.pi*(53*t+1.4*(1-math.exp(-46*t))))
        value=thump+.27*lp*math.exp(-46*t)
        # Boards snap into the tray, then the tine rattles against the pallet.
        for onset,hz,gain in [(0,633,.40),(.033,417,.53),(.086,720,.39),
                              (.140,324,.42),(.236,491,.24),(.319,699,.15)]:
            u=t-onset
            if u<0:continue
            resonant=math.sin(2*math.pi*hz*u)+.28*math.sin(2*math.pi*hz*2.41*u)
            value+=gain*(.72*math.exp(-37*u)*resonant+.41*math.exp(-110*u)*lp)
        return .9*value
    return effect(.55,sample,884)


def clock_out():
    # A punch clock stamps, then its reeds answer with E-G-B-D-E.
    def sample(t,rng):
        value=.13*math.exp(-140*t)*rng.uniform(-1,1)
        for onset,pitch in [(0,64),(.102,67),(.189,71),(.296,74),(.432,76)]:
            u=t-onset
            if u<0:continue
            f=440*2**((pitch-69)/12)
            phase=2*math.pi*f*u
            env=min(1,620*u)*math.exp(-(7.4 if pitch==76 else 14)*u)
            value+=.48*env*(.71*math.sin(phase)+.23*math.sin(2*phase)+.12*math.sin(2.75*phase))
        return value
    return effect(.92,sample,17)


if __name__ == '__main__':
    write_wad(Path(__file__).with_name('fork-music.wad'),
              {b'D_E1M9':music(), b'DSFRBEEP':beep(), b'DSFRHYD':hydraulics(),
               b'DSFRSMAS':cargo_smash(), b'DSFRCLEA':clock_out()})
