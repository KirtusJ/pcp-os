#!/usr/bin/env python3
"""The PCP OS kernel's tests that need more than the kernel: an emulator, programs to load, and for Doom, game data.

    python3 tests/run_tests.py <kernel elf32> <dir with doom.elf, probe.elf> <dir with the game data> <output dir> [quick]

Each test starts the kernel in QEMU, drives it as a person or a loader would (keys through the emulator's
monitor, programs and arguments through the loader's module list and command line), and reads three things:
the kernel's serial log, pictures of the display, and the sound the emulated sound devices produced. A
test's expectations are in this file, written before the run. The results are written as JSON beside the
pictures. Nothing here is a measurement on hardware: the processor is emulated in software.
"""
import hashlib, json, math, os, shutil, socket, struct, subprocess, sys, time, wave
from pathlib import Path

KERNEL, PROGRAMS, DATA, OUT = (Path(a).resolve() for a in sys.argv[1:5])
QUICK = len(sys.argv) > 5 and sys.argv[5] == "quick"
OUT.mkdir(parents=True, exist_ok=True)
RESULTS = []
ONLY = os.environ.get("PCPOS_TESTS_ONLY")   # run only the tests whose name contains this: for use while working, not for a record

class Machine:
    def __init__(self, name, modules=(), append="", cpu="max", sound=False, display=True, vram_mib=None):
        self.dir = OUT / name
        if self.dir.exists(): shutil.rmtree(self.dir)
        self.dir.mkdir(parents=True)
        self.serial = self.dir / "serial.log"; self.wav = self.dir / "sound.wav"
        cmd = ["qemu-system-x86_64", "-cpu", cpu, "-m", "256M", "-kernel", str(KERNEL), "-display", "none", "-vga", "std" if display else "none",
               "-serial", "file:" + str(self.serial), "-no-reboot", "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04", "-qmp", "unix:qmp,server=on,wait=off"]
        if vram_mib is not None: cmd += ["-global", f"VGA.vgamem_mb={vram_mib}"]
        if modules: cmd += ["-initrd", ",".join(str(m) for m in modules)]
        if append: cmd += ["-append", append]
        if sound: cmd += ["-audiodev", "wav,id=snd0,path=%s" % self.wav, "-device", "AC97,audiodev=snd0", "-device", "adlib,audiodev=snd0"]
        # The emulator is started in the test's own folder and its monitor socket is named relative to it: a
        # socket's full path may be no longer than about a hundred characters, and a results folder's often is.
        self.started = time.time(); self.p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, cwd=self.dir)
        here = os.open(self.dir, os.O_RDONLY)
        for _ in range(200):
            try:
                self.q = socket.socket(socket.AF_UNIX); self.q.connect("/proc/self/fd/%d/qmp" % here); break
            except OSError: time.sleep(0.05)
        os.close(here)
        self.f = self.q.makefile("rwb"); self._read(); self.qmp("qmp_capabilities")
    def _read(self):
        while True:
            j = json.loads(self.f.readline())
            if "event" not in j: return j
    def qmp(self, execute, **arguments):
        self.f.write(json.dumps({"execute": execute, "arguments": arguments}).encode() + b"\n"); self.f.flush(); return self._read()
    def picture(self, name):
        from PIL import Image
        ppm = self.dir / (name + ".ppm"); self.qmp("screendump", filename=str(ppm)); time.sleep(0.15)
        im = Image.open(ppm).convert("RGB"); im.save(self.dir / (name + ".png")); ppm.unlink(); return im
    def key(self, *names, hold=80):
        self.qmp("send-key", keys=[{"type": "qcode", "data": n} for n in names], **{"hold-time": hold}); time.sleep(hold / 1000 + 0.05)
    def hold(self, name, seconds):
        ev = lambda down: self.qmp("input-send-event", events=[{"type": "key", "data": {"down": down, "key": {"type": "qcode", "data": name}}}])
        ev(True); time.sleep(seconds); ev(False)
    def log(self): return self.serial.read_text(errors="replace") if self.serial.exists() else ""
    def wait_log(self, text, timeout=60):
        t0 = time.time()
        while time.time() - t0 < timeout:
            if text in self.log(): return True
            if self.p.poll() is not None: return text in self.log()
            time.sleep(0.1)
        return False
    def end(self, timeout=60):
        try: rc = self.p.wait(timeout)
        except subprocess.TimeoutExpired: self.p.kill(); self.p.wait(); rc = None
        return rc, self.log()

def colours(im):
    b = im.resize((160, 100)).tobytes(); return len({b[i:i + 3] for i in range(0, len(b), 3)})
def differ(a, b):
    if a.size != b.size: return 1.0
    pa, pb = a.resize((160, 100)).tobytes(), b.resize((160, 100)).tobytes()
    return sum(1 for i in range(0, len(pa), 3) if pa[i:i+3] != pb[i:i+3]) / (len(pa) / 3)
def sound_by_second(path):
    w = wave.open(str(path)); n = w.getnframes(); raw = w.readframes(n); s = struct.unpack("<%dh" % (len(raw) // 2), raw); per = w.getframerate() * w.getnchannels()
    return [round(math.sqrt(sum(x * x for x in s[t:t + per]) / max(1, len(s[t:t + per])))) for t in range(0, len(s), per)]

def test(name):
    def deco(fn):
        if ONLY and ONLY not in name: return fn
        t0 = time.time(); checks = []
        def ck(what, ok, detail=""):
            checks.append({"check": what, "ok": bool(ok), "detail": "" if ok else str(detail)[:500]})
        try: facts = fn(ck) or {}
        except Exception as e:
            import traceback; facts = {}; checks.append({"check": "the test ran", "ok": False, "detail": traceback.format_exc()[-900:]})
        ok = bool(checks) and all(c["ok"] for c in checks)
        RESULTS.append({"test": name, "ok": ok, "seconds": round(time.time() - t0, 1), "checks": checks, "facts": facts})
        print(("pass " if ok else "FAIL ") + name + "".join("\n      FAIL %s | %s" % (c["check"], c["detail"]) for c in checks if not c["ok"]), flush=True)
        if facts: print("      " + json.dumps(facts)[:600], flush=True)
        return fn
    return deco

def ended(log, name):
    for l in log.splitlines():
        if l.startswith(name + " ended ") or (name + " ended ") in l: return l[l.index(name + " ended "):]
    return ""
def verdict(ck, rc, log):
    ck("the kernel stopped in order with every self-test passed (exit status 33)", rc == 33 and "ALL SELF-TESTS PASS" in log and "  FAIL" not in log, (rc, [l for l in log.splitlines() if "FAIL" in l or "STOPPED:" in l][:4]))
    ck("the kernel's register verifies and replays at the end", "replays without a problem" in log and "pass the kernel's register of its own start" in log)

DOOM, PROBE = PROGRAMS / "doom.elf", PROGRAMS / "probe.elf"
SHAREWARE, FREEDOOM = DATA / "doom1.wad", DATA / "freedoom1.wad"

# ---- the kernel alone --------------------------------------------------------------------------------------
for cpu in ("max", "qemu64"):
    @test("the kernel alone, processor model %s: self-tests, nothing to run, stops" % cpu)
    def _(ck, cpu=cpu):
        m = Machine("alone_" + cpu, append="then=off", cpu=cpu); rc, log = m.end(120); verdict(ck, rc, log)
        n = log.count("\n  pass "); ck("at least 58 self-tests are reported", n >= 58, n)
        return {"self_tests_passed": n, "seconds_to_stop": round(time.time() - m.started, 1)}

# ---- the image the kernel measures, against a value it did not compute (K-12, since build 22) -----------------
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools")); import measure
EXPECTED = measure.measure(KERNEL.read_bytes())     # from the image file, without running it

@test("the kernel's measurement of itself equals the value computed from the image file, and it verifies when that value is named")
def _(ck):
    m = Machine("image_named", append="then=off image=" + EXPECTED); rc, log = m.end(120); verdict(ck, rc, log)
    said = [l for l in log.splitlines() if "running image" in l]
    ck("what the running kernel measured is what the tool computed from the file", bool(said) and EXPECTED in said[0] and "equal to the value the loader named" in said[0], said[:1])
    ck("the comparison is one of its self-tests, and passed", "pass the running image measures as the value the loader named" in log)
    return {"image_measurement": EXPECTED, "self_tests_passed": log.count("\n  pass ")}

@test("named another value, the kernel does not verify: it says so, is in SAFE, and starts no program")
def _(ck):
    wrong = hashlib.sha256(b"another image").hexdigest()
    m = Machine("image_other", modules=["%s granted" % PROBE], append="auto=probe.elf image=" + wrong); rc, log = m.end(120)
    ck("it stops with a status that is not the one for a pass", rc != 33 and "SELF-TEST(S) FAILED" in log and "ALL SELF-TESTS PASS" not in log, rc)
    ck("it says which value was named and that its own differs", "NOT the value the loader named, which is " + wrong in log and EXPECTED in log)
    ck("that comparison is the self-test that failed, and the only one", log.count("\n  FAIL ") == 1 and "\n1 SELF-TEST(S) FAILED" in log and "FAIL the running image measures as the value the loader named" in log, [l for l in log.splitlines() if "FAIL" in l][:4])
    ck("the processes built into the kernel were not run either", "the processes built into it are not run" in log and "every process is a task" not in log)
    ck("no program was started", "starts no program" in log and "auto=probe.elf: not started" in log and "starting probe.elf" not in log and "probe.elf ended" not in log, [l for l in log.splitlines() if "probe" in l][:4])
    ck("its register still closes and replays: verification ended in SAFE and nothing else", "replays without a problem" in log)
    return {"exit_status": rc}

@test("with a display and a keyboard and nothing to run, the kernel shows its menu and switches off when asked")
def _(ck):
    m = Machine("menu_only"); ck("devices were found", m.wait_log("music:", 60)); time.sleep(1.5)
    im = m.picture("menu"); ck("the desktop is drawn at 1920 by 1080 with the emblem's colours", im.size == (1920, 1080) and colours(im) > 20, (im.size, colours(im)))
    m.key("r"); time.sleep(0.8); reg = m.picture("register"); ck("R shows the register", differ(im, reg) > 0.02, differ(im, reg))
    m.key("esc"); time.sleep(0.5); m.key("p"); rc, log = m.end(30); verdict(ck, rc, log)

# ---- programs that are not Doom ------------------------------------------------------------------------------
@test("the desktop negotiates two-page modes and a separate UI scale")
def _(ck):
    for mb, size, scale in [(32, (2560, 1440), 2), (4, (800, 600), 1)]:
        m = Machine(f"desktop_{mb}MiB", append="desktop=2560x1440 ui_scale=2", vram_mib=mb)
        marker = f"desktop: {size[0]}x{size[1]} at {scale}x UI scale"
        ck(f"{mb} MiB reports its actual mode and scale", m.wait_log(marker, 60))
        time.sleep(1.5)
        image = m.picture("desktop")
        ck(f"{mb} MiB guest framebuffer", image.size == size and colours(image) > 20, image.size)
        m.key("p"); rc, log = m.end(30); verdict(ck, rc, log)

@test("a C program built with an ordinary compiler and C library runs with every grant: 26 checks of its own")
def _(ck):
    m = Machine("probe_granted", modules=["%s granted" % PROBE], append="auto=probe.elf"); rc, log = m.end(60); verdict(ck, rc, log)
    ck("every one of its checks is ok", log.count("\nok  ") == 26 and "probe: 0 failure(s)" in log, [l for l in log.splitlines() if l.startswith("FAIL")])
    e = ended(log, "probe.elf"); ck("it ended DONE and all its memory came back", e.startswith("probe.elf ended DONE") and "0 both writable and executable" in e, e[:200])

@test("the same program with no grant but the console is refused every device and file: 18 checks")
def _(ck):
    m = Machine("probe_ungranted", modules=[PROBE], append="auto=probe.elf grant=none"); rc, log = m.end(60); verdict(ck, rc, log)
    ck("every one of its checks is ok", log.count("\nok  ") == 18 and "probe: 0 failure(s)" in log, [l for l in log.splitlines() if l.startswith("FAIL")])
    ck("five of them are refusals for want of a grant", log.count("ok   without the grant") == 5)

@test("a loaded program that writes to the kernel's memory is ended by the kernel, and the kernel goes on")
def _(ck):
    m = Machine("probe_fault", modules=["%s granted fault" % PROBE], append="auto=probe.elf"); rc, log = m.end(60); verdict(ck, rc, log)
    e = ended(log, "probe.elf"); ck("ended FAILED by the processor's refusal, at the kernel address it tried", "ended FAILED" in e and "exception 14" in e and "address 0x101000" in e, e[:240])

def variants():
    """Files that are not programs this kernel may run, made from one that is."""
    good = PROBE.read_bytes(); d = OUT / "bad"; d.mkdir(exist_ok=True); out = []
    phoff, phnum = struct.unpack_from("<Q", good, 32)[0], struct.unpack_from("<H", good, 56)[0]
    b = bytearray(good)
    for n in range(phnum):
        h = phoff + n * 56
        if struct.unpack_from("<I", b, h)[0] == 1 and struct.unpack_from("<I", b, h + 4)[0] & 2: struct.pack_into("<I", b, h + 4, 7)
    out.append(("wx.elf", bytes(b), "a segment asks to be both writable and executable"))
    b = bytearray(good); struct.pack_into("<H", b, 18, 183); out.append(("arm.elf", bytes(b), "not a 64-bit little-endian program for x86_64"))
    b = bytearray(good); struct.pack_into("<H", b, 16, 2); out.append(("fixed.elf", bytes(b), "a segment lies outside the memory a program may have"))
    out.append(("short.elf", good[:200], "program headers are missing or lie outside the file"))
    b = bytearray(good); struct.pack_into("<Q", b, 24, 0x7fffffff0000); out.append(("entry.elf", bytes(b), "the entry address is not in executable memory of the program"))
    out.append(("dynamic.elf", Path("/bin/true").read_bytes(), "the program asks for a dynamic loader"))
    for name, data, _ in out: (d / name).write_bytes(data)
    return [(d / name, why) for name, _, why in out]

@test("six files that are not programs this kernel may run are each refused, with the reason, before anything of theirs runs")
def _(ck):
    for path, why in variants():
        m = Machine("bad_" + path.stem, modules=[path], append="auto=" + path.name); rc, log = m.end(60)
        ck("%s: refused because %s" % (path.name, why), ("%s could not be started: %s" % (path.name, why)) in log, [l for l in log.splitlines() if "could not" in l or "ended" in l][:2])
        ck("%s: the kernel stopped in order" % path.name, rc == 33 and "ALL SELF-TESTS PASS" in log, rc)

# ---- Doom ------------------------------------------------------------------------------------------------------
def timedemo(ck, wad, demo, tag):
    m = Machine("timedemo_%s_%s" % (tag, demo), modules=["%s -timedemo %s" % (DOOM, demo), wad], append="auto=doom.elf"); rc, log = m.end(600); verdict(ck, rc, log)
    line = next((l for l in log.splitlines() if "gametics in" in l), "")
    try:
        tics, real = int(line.split("timed ")[1].split()[0]), int(line.split(" in ")[1].split()[0]); fps = tics * 35.0 / real
    except Exception: tics = real = 0; fps = 0.0
    ck("the engine reports its own timing", tics > 0, line[-120:])
    ck("at least three times the 35 pictures a second the game needs", fps >= 105, fps)
    return {"game_data": Path(wad).name, "demo": demo, "gametics": tics, "realtics": real, "pictures_a_second": round(fps, 1)}

for demo in (("demo1", "demo2", "demo3") if not QUICK else ("demo3",)):
    @test("Doom, shareware data: the engine's own benchmark on %s (every picture drawn, as fast as it will go)" % demo)
    def _(ck, demo=demo): return timedemo(ck, SHAREWARE, demo, "shareware")
if FREEDOOM.exists():
    for demo in (("demo1", "demo2", "demo3") if not QUICK else ("demo1",)):
        @test("Doom, Freedoom data: the engine's own benchmark on %s" % demo)
        def _(ck, demo=demo): return timedemo(ck, FREEDOOM, demo, "freedoom")

@test("Doom at the speed of play: a recorded demo of 2134 game tics plays for 61 seconds by the clock")
def _(ck):
    m = Machine("playdemo", modules=["%s -playdemo demo3" % DOOM, SHAREWARE], append="auto=doom.elf", sound=True); rc, log = m.end(200); verdict(ck, rc, log)
    e = ended(log, "doom.elf")
    try: secs = int(e.split(" after ")[1].split(" s")[0]); shown = int(e.split(" frames shown")[0].split()[-1]); idle = int(e.split("idled ")[1].split("%")[0]); dry = int(e.split("ran out of sound ")[1].split()[0])
    except Exception: secs = shown = idle = 0; dry = -1
    ck("it ran between 61 and 67 seconds (2134 tics at 35 a second is 61 s; the rest is start and the melt)", 61 <= secs <= 67, e[:160])
    ck("at least 35 pictures a second were shown", secs and shown / secs >= 35, (shown, secs))
    ck("the processor was idle more than half the time", idle > 50, idle)
    ck("the sound device never ran out of sound", dry == 0, dry)
    return {"seconds": secs, "pictures_shown": shown, "pictures_a_second": round(shown / max(secs, 1), 1), "processor_idle_percent": idle, "sound_ran_dry": dry}

@test("Doom as a person plays it: menu, start, new game, walk, turn, fire, save, load, stop key, start again, quit from its own menu")
def _(ck):
    m = Machine("play", modules=[DOOM, SHAREWARE], sound=True)
    ck("the kernel found display, keyboard, sound and music", m.wait_log("music: an FM synthesiser", 60), m.log()[-300:]); time.sleep(1.5)
    menu = m.picture("1_menu"); ck("the kernel's menu lists the program", colours(menu) > 20 and menu.size == (1920, 1080))
    m.key("ret"); time.sleep(1.5)   # the desktop is empty: Enter opens the launcher, and Enter there starts the program
    m.key("ret"); ck("Enter starts it", m.wait_log("doom on PCP OS: display 640x400", 30)); time.sleep(7)
    title = m.picture("2_title"); ck("the game draws at 640 by 400 with many colours", title.size == (640, 400) and colours(title) > 100, (title.size, colours(title)))
    for k in ("esc", "ret", "ret", "ret"): m.key(k); time.sleep(0.7)
    time.sleep(3.5)
    start = m.picture("3_level"); time.sleep(0.3)
    m.hold("up", 1.5); walked = m.picture("4_walked"); ck("holding Up walks: the view changes", differ(start, walked) > 0.2, differ(start, walked))
    m.hold("left", 0.6); turned = m.picture("5_turned"); ck("holding Left turns: the view changes", differ(walked, turned) > 0.2, differ(walked, turned))
    bar = lambda im: im.crop((0, 336, 110, 400))
    before = m.picture("6_before_fire")
    for _ in range(5): m.key("ctrl"); time.sleep(0.45)
    after = m.picture("7_after_fire"); ck("Ctrl fires: the ammunition count on the status bar changes", differ(bar(before), bar(after)) > 0.005, differ(bar(before), bar(after)))
    # save in the first slot under the name "t", walk on, load it again
    m.key("f2"); time.sleep(0.6); m.key("ret"); time.sleep(0.4); m.key("t"); time.sleep(0.3); m.key("ret"); time.sleep(1.0)
    saved = m.picture("8_saved"); m.hold("up", 1.2); moved = m.picture("9_moved_on")
    m.key("f3"); time.sleep(0.6); m.key("ret"); time.sleep(2.5); loaded = m.picture("10_loaded")
    ck("after loading, the view is the saved one and not the later one", differ(saved, loaded) < differ(moved, loaded) and differ(saved, loaded) < 0.25, (differ(saved, loaded), differ(moved, loaded)))
    m.key("ctrl", "alt", "delete"); time.sleep(2.0)
    back = m.picture("11_back"); ck("Ctrl+Alt+Delete returns to the desktop mode", back.size == menu.size and "stopped by the person" in m.log(), back.size)
    e1 = ended(m.log(), "doom.elf"); ck("the stopped run is recorded STOPPED and left the saved game in the store", "ended STOPPED" in e1 and "the store holds 1 file(s)" in e1, e1[-120:])
    m.key("ret"); time.sleep(8); again = m.picture("12_again"); ck("it starts again in the same session", again.size == (640, 400) and colours(again) > 100)
    for k in ("esc", "up", "ret"): m.key(k); time.sleep(0.6)
    m.key("y"); time.sleep(4.0)
    log = m.log(); second = log[log.rindex("starting doom.elf"):] if "starting doom.elf" in log else ""
    ck("quitting from the game's own menu ends it DONE (exit 0)", "doom.elf ended DONE: exit 0" in second, ended(second, "doom.elf")[:120])
    # (The engine as doomgeneric builds it does not write its settings file: that code is compiled out there.)
    e2 = ended(second, "doom.elf"); ck("the saved game is still in the store after the second run: the store lasts from one program to the next", "the store holds 1 file(s)" in e2, e2[-80:])
    time.sleep(1.0); m.key("p"); rc, log = m.end(30); verdict(ck, rc, log)
    lev = sound_by_second(m.wav); loud = sum(1 for v in lev if v > 200)
    ck("sound was produced for most of the time the game ran", loud >= 20, lev)
    dry = [int(l.split("ran out of sound ")[1].split()[0]) for l in log.splitlines() if "ran out of sound" in l]
    late = [int(l.split("the latest ")[1].split()[0]) for l in log.splitlines() if "the latest " in l]
    ck("music register writes were never more than 100 ms late", late and max(late) <= 100, late)
    return {"seconds_with_sound": loud, "seconds_recorded": len(lev), "sound_ran_dry_each_run": dry, "music_latest_ms": late, "pictures": sorted(p.name for p in m.dir.glob("*.png"))}

@test("Doom, Freedoom data, left to itself for %d seconds: no fault, nothing lost" % (60 if QUICK else 240))
def _(ck):
    wad = FREEDOOM if FREEDOOM.exists() else SHAREWARE
    m = Machine("soak", modules=[DOOM, wad], append="auto=doom.elf then=menu", sound=True)
    ck("it started", m.wait_log("doom on PCP OS: display", 60)); pics = []
    for n in range(4):
        time.sleep((60 if QUICK else 240) / 4); pics.append(m.picture("soak_%d" % n))
    ck("the picture keeps changing", all(differ(pics[i], pics[i + 1]) > 0.05 for i in range(3)), [differ(pics[i], pics[i + 1]) for i in range(3)])
    m.key("ctrl", "alt", "delete"); time.sleep(2); m.key("p"); rc, log = m.end(30); verdict(ck, rc, log)
    e = ended(log, "doom.elf"); ck("its memory all came back", " free before and " in e and e.split(" free before and ")[0].split()[-1] == e.split(" free before and ")[1].split()[0], e[-160:])
    try: idle = int(e.split("idled ")[1].split("%")[0]); shown = int(e.split(" frames shown")[0].split()[-1]); secs = int(e.split(" after ")[1].split(" s")[0])
    except Exception: idle = shown = secs = 0
    return {"game_data": wad.name, "seconds": secs, "pictures_shown": shown, "processor_idle_percent": idle, "unanswered_calls": [l for l in log.splitlines() if "does not answer" in l][:1]}

@test("Doom with no display: it says so and exits, and the kernel goes on")
def _(ck):
    m = Machine("no_display", modules=[DOOM, SHAREWARE], append="auto=doom.elf", display=False); rc, log = m.end(60); verdict(ck, rc, log)
    ck("the program reported that there is no display it may use", "there is no display this program may use" in log and "doom.elf ended FAILED: exit 2" in log, ended(log, "doom.elf")[:100])

summary = {"kernel_sha256": hashlib.sha256(KERNEL.read_bytes()).hexdigest(), "doom_sha256": hashlib.sha256(DOOM.read_bytes()).hexdigest(),
           "game_data": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (SHAREWARE, FREEDOOM) if p.exists()},
           "emulator": subprocess.run(["qemu-system-x86_64", "--version"], capture_output=True, text=True).stdout.splitlines()[0], "hardware_virtualization": os.path.exists("/dev/kvm"),
           "tests": len(RESULTS), "passed": sum(r["ok"] for r in RESULTS), "checks": sum(len(r["checks"]) for r in RESULTS), "results": RESULTS}
(OUT / "results.json").write_text(json.dumps(summary, indent=1))
print("tests %d, passed %d, checks %d" % (summary["tests"], summary["passed"], summary["checks"]))
sys.exit(0 if summary["passed"] == summary["tests"] else 1)
