"""Compose Last Freight to Lucky Seven and five original train sounds. Python 3, standard library."""
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


BASS, HARP, PIANO, GUITAR, ORGAN = range(5)
# Two choruses of G minor blues either side of the eight-bar station stop.
BLUES = (43, 43, 43, 43, 48, 48, 43, 43, 51, 50, 43, 50)
BRIDGE = (43, 43, 46, 46, 48, 48, 50, 50)
LUCKY = ((0,67,.40), (.5,70,.17), (.75,72,.62), (1.5,73,.18),
         (1.75,74,.47), (2.5,65,.31), (3,67,.81))

# Lead sheets use scale intervals, in semitones, from the current root.
# The long-short pairs land on the rolling triplet eighths.
HARP_LINES = (
    ((0, 7, .52), (2/3, 10, .21), (1, 12, 1.13), (2+1/3, 10, .24), (3, 7, .70)),
    ((1/3, 3, .37), (1, 5, .43), (1+2/3, 6, .17), (2, 7, 1.38)),
    ((0, 12, .81), (1, 10, .41), (1+2/3, 7, .23), (2, 5, .65), (3, 3, .60)),
    ((2/3, 3, .22), (1, 0, 1.16), (2+2/3, 5, .23), (3, 7, .64)),
)
DOMINANT = ((0, 4, .6), (1, 7, .47), (1+2/3, 10, .25), (2, 12, .61), (3, 10, .51))


def backbeat(s, t, bar, in_station):
    for beat in range(4):
        # Steam valve opening, then the shoe coming down on each beat.
        velocity = (76, 50, 67, 54)[beat] - 13 * in_station
        s.note(DRUMS, t + beat, 44, .10, velocity)
        if beat in (0, 2):
            s.note(DRUMS, t + beat, 36, .15, 76 if in_station else 89)
        else:
            s.note(DRUMS, t + beat + .035, 37, .10, 90)
        if not in_station:
            s.note(DRUMS, t + beat + 2/3, 42, .11, 48 + beat % 2 * 8)
    if bar in (3, 11, 19, 27, 31):
        for off, drum in ((2+2/3, 50), (3+1/3, 47), (3+2/3, 45)):
            s.note(DRUMS, t + off, drum, .15, 76)


def boogie():
    s = Score(62, [(BASS, 32, 102, 64), (HARP, 22, 95, 70),
                   (PIANO, 3, 91, 44), (GUITAR, 25, 72, 81), (ORGAN, 18, 92, 59)])
    changes = BLUES + BRIDGE + BLUES
    for bar, root in enumerate(changes):
        t = 4*bar
        bridge = 12 <= bar < 20
        sparse = 12 <= bar < 16
        cell = bar if bar < 12 else bar - (12 if bridge else 20)
        third = 4 if root in (50, 51, 46) else 3
        next_root = changes[(bar+1) % len(changes)]
        # Walking line and a late, swung pickup across the drawbar.
        walk = (0, 7, 12, 7) if cell % 2 == 0 else (0, third, 5, 7)
        for beat, interval in enumerate(walk):
            if sparse and beat % 2:
                continue
            s.note(BASS, t+beat, root-12+interval, .47 if not sparse else 1.1,
                   100 if beat == 0 else 86)
            if not sparse and (beat == 3 or (beat == 1 and cell % 2 == 0)):
                pitch = next_root-13 if beat == 3 else root-3
                s.note(BASS, t+beat+2/3, pitch, .23, 74)
        backbeat(s, t, bar, sparse)
        for off in ((1+2/3,) if sparse else (2/3, 2+2/3)):
            s.note(GUITAR, t+off, root+third, .18, 76)
            s.note(GUITAR, t+off+.018, root+10, .21, 63)
        if bridge:
            if cell in (0, 2):
                # Lucky still has the seven-note organ ticket from the alley.
                s.phrase(ORGAN, t, LUCKY, 93, root-43)
            elif cell in (1, 3):
                s.phrase(PIANO, t, ((1, 62, .41), (1+2/3, 65, .25), (2, 67, 1.05)), 82)
            else:
                s.phrase(HARP, t, DOMINANT if root == 50 else HARP_LINES[cell%4],
                         90, root+12)
        elif bar >= 2:
            lead = DOMINANT if root == 50 else HARP_LINES[cell%4]
            s.phrase(HARP, t, lead, 96 if bar >= 20 else 88, root+12)
            if cell % 4 in (1, 3):
                s.note(PIANO, t+2+2/3, root+19, .24, 78)
                s.note(PIANO, t+3, root+12+third, .53, 82)
        else:
            phrase = ((0, 0, .52), (2/3, 3, .24), (1, 5, .51),
                      (1+2/3, 6, .22), (2, 7, .8), (3, 10, .54))
            s.phrase(PIANO, t, phrase, 95 if bar == 0 else 78, root+12*(1+bar))
    return s.mus(128)


def whistle():
    # Three pipes speak successively as the valve opens. The little portamento
    # on release comes from the last of the steam escaping.
    def sample(t, rng):
        vent = min(1, 14*t) * min(1, max(0, (.89-t)*7))
        sound = .09 * rng.uniform(-1,1) * vent
        for delay, hz, level in ((0,392,.31), (.03,466,.25), (.065,588,.20)):
            u = t - delay
            if u < 0:
                continue
            drop = max(0, u-.59)
            phase = 2*math.pi * (hz*u - 2.5*(1-math.exp(-15*u)) - 62*drop*drop
                                 + .048*math.sin(2*math.pi*6*u))
            sound += level * vent * min(1, 16*u) * (math.sin(phase) + .20*math.sin(2*phase))
        return sound
    return effect(.93, sample, 87)


def impact(t, onset, freq, decay):
    u = t - onset
    if u < 0:
        return 0
    phase = 2*math.pi*freq*u
    return min(1, 960*u)*math.exp(-decay*u)*(math.sin(phase)+.30*math.sin(2.61*phase))


def interlock():
    def sample(t, rng):
        return (.4*impact(t,0,714,53) + .46*impact(t,.064,288,25)
                + .25*impact(t,.079,982,52) + .13*rng.uniform(-1,1)*math.exp(-51*t))
    return effect(.26, sample, 206)


def brakes():
    lowpass = 0
    def sample(t, rng):
        nonlocal lowpass
        noise = rng.uniform(-1,1)
        lowpass = .91*lowpass + .09*noise
        air = .34*(noise-lowpass)*min(1,16*t)*math.exp(-2.5*t)
        envelope = min(1,8*t)*min(1,max(0,(.71-t)*8))
        phase = 2*math.pi*(912*t - 350*t*t + .18*math.sin(2*math.pi*23*t))
        friction = .34*envelope*(math.sin(phase) + .26*math.sin(3*phase))
        shoe = .26*impact(t,.68,233,30)
        return air + friction + shoe
    return effect(.91, sample, 45)


def station_bell():
    # The conductor gave the barrel its own stop. It always rolls past.
    def sample(t, rng):
        sound = 0
        for onset, volume in ((0,.66), (.26,.49)):
            u = t-onset
            if u >= 0:
                for hz, decay, weight in ((660,3.5,.52), (1329,5,.22), (1819,6,.14),
                                          (2392,10,.12), (3301,13,.07)):
                    sound += volume*weight*impact(t,onset,hz,decay)
        return sound
    return effect(1.16, sample, 201)


def rail_joint():
    def sample(t, rng):
        value = 0
        for onset, gain in ((0,.68), (.073,.50)):
            u = t-onset
            if u >= 0:
                value += gain*(.53*impact(t,onset,167,52) + .27*impact(t,onset,555,68)
                               + .33*rng.uniform(-1,1)*math.exp(-96*u))
        return value
    return effect(.17, sample, 717)


if __name__ == '__main__':
    write_wad(Path(__file__).with_name('rail-music.wad'),
              {b'D_E1M9': boogie(), b'DSRWHIST': whistle(), b'DSRLOCK': interlock(),
               b'DSRBRAKE': brakes(), b'DSRBELL': station_bell(), b'DSRWHEEL': rail_joint()})
