"""Candidate firmware tests use the 1024×768 Details view for comparable register captures."""

from pathlib import Path
import runpy

OPTIONS = "first=details desktop=1024x768"

# The selected region on the current Details screen includes the D desktop and P off
# hints. The historical register footer and both sequence-number column hashes still match.
FOOTER = (12, 735, 895, 760, "2f75c23ac365a0a4a3a70a0ac590b1f3892b1c471b68dd71b1f21d8751524888")


def monitor_for(vm):
    optical = runpy.run_path(str(Path(__file__).resolve().parents[1] / "optical-disk-check/check.py"))
    monitor = optical["Monitor"](vm)
    monitor.call("qmp_capabilities")
    return monitor


def power_off(monitor, vm):
    monitor.key("ret")
    vm.wait(timeout=30)
    while not any(event["event"] == "SHUTDOWN" for event in monitor.events):
        event = monitor.read()
        if "event" in event:
            monitor.events.append(event)
    shutdown = next(event["data"] for event in monitor.events if event["event"] == "SHUTDOWN")
    if vm.returncode != 0 or shutdown.get("guest") is not True or shutdown.get("reason") != "guest-shutdown":
        raise RuntimeError(f"Unexpected QEMU termination: {vm.returncode}, {shutdown}")
    return shutdown


def finish_session(call, until, log, out, public):
    """Run the candidate Details UI, check the register, cancel power and restore Details."""
    def press(*keys):
        call("send-key", keys=[{"type": "qcode", "data": k} for k in keys], **{"hold-time": 70})
    def tap(key):
        call("input-send-event", events=[
            {"type":"key","data":{"down":down,"key":{"type":"qcode","data":key}}}
            for down in (True,False)])
    def frame(label, regions, reference=None):
        path = out / (label + ".ppm")
        original = (out / (reference + ".ppm")).read_bytes() if reference else None
        def matches():
            call("screendump", filename=str(path))
            raw = path.read_bytes()
            return public.has_regions(raw, regions) and (original is None or raw == original)
        until(matches, label)
    frame("ready", (FOOTER,))
    press("ret")
    until(lambda: "SURFBIRD media ready" in log(), "media menu")
    press("spc")
    until(lambda: "SURFBIRD media drained" in log() and "surf.elf ended DONE" in log(), "normal exit")
    frame("after-playback", (FOOTER,))
    press("ret")
    until(lambda: log().count("SURFBIRD media ready") == 2, "second launch")
    press("ctrl", "alt", "delete")
    until(lambda: "surf.elf ended STOPPED" in log(), "keyboard Stop")
    frame("after-stop", (FOOTER,))
    tap("r")
    frame("register-latest", (public.REGISTER_FOOTER, public.LATEST_ROWS))
    tap("up")
    frame("register-earlier", (public.REGISTER_FOOTER, public.EARLIER_ROWS))
    tap("down")
    frame("register-restored", (public.REGISTER_FOOTER, public.LATEST_ROWS), "register-latest")
    tap("esc")
    frame("after-register", (FOOTER,), "after-stop")

    reader = runpy.run_path(str(Path(__file__).resolve().parents[1] / "tester-cd/check.py"))
    glyphs = reader["kernel_glyphs"]()
    prompt = {1: "POWER OFF", 3: "Switch PCP OS off?",
              5: "The register is closed and verified first. Files of this session that were",
              6: "not saved by a program are lost.", 36: "Enter switch off    Esc go back"}
    def ask(label):
        tap("p")
        path = out / (label + ".ppm")
        def visible():
            call("screendump", filename=str(path))
            rows = reader["read_screen"](path.read_bytes(), glyphs)
            return rows and len(rows) == 38 and all(rows[i].strip() == text for i, text in prompt.items())
        until(visible, label)
    ask("power-prompt")
    tap("esc")
    frame("power-cancelled", (FOOTER,), "after-register")
    ask("power-prompt-confirm")
    # The caller records its final firmware checks before issuing the fresh Enter.
