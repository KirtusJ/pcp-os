//! Launch one Store guest. QEMU receives short filenames so all measured boot words fit.
use pcpos_core::sha256_hex;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};
fn failure(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn measurement(b: &[u8]) -> io::Result<String> {
    let u16at = |i: usize| {
        b.get(i..i + 2)
            .map(|v| u16::from_le_bytes(v.try_into().unwrap()) as usize)
    };
    let u32at = |i: usize| {
        b.get(i..i + 4)
            .map(|v| u32::from_le_bytes(v.try_into().unwrap()) as usize)
    };
    let err = || failure("Invalid PCP OS ELF32 kernel section table");
    if b.get(..6) != Some(b"\x7fELF\x01\x01") {
        return Err(err());
    }
    let off = u32at(32).ok_or_else(err)?;
    let stride = u16at(46).ok_or_else(err)?;
    let count = u16at(48).ok_or_else(err)?;
    if stride < 40
        || off
            .checked_add(count.checked_mul(stride).ok_or_else(err)?)
            .is_none_or(|i| i > b.len())
    {
        return Err(err());
    }
    let section = |i: usize| -> io::Result<(usize, usize, &[u8])> {
        if i >= count {
            return Err(err());
        }
        let h = off + i * stride;
        let start = u32at(h + 16).ok_or_else(err)?;
        let size = u32at(h + 20).ok_or_else(err)?;
        Ok((
            u32at(h).ok_or_else(err)?,
            u32at(h + 12).ok_or_else(err)?,
            b.get(start..start.checked_add(size).ok_or_else(err)?)
                .ok_or_else(err)?,
        ))
    };
    let (_, _, names) = section(u16at(50).ok_or_else(err)?)?;
    let (mut code, mut ro) = (None, None);
    for i in 0..count {
        let h = off + stride * i;
        let name = u32at(h)
            .and_then(|k| names.get(k..))
            .and_then(|s| s.split(|b| *b == 0).next())
            .ok_or_else(err)?;
        match name {
            b".text" => code = Some(section(i)?),
            b".rodata" => ro = Some(section(i)?),
            _ => {}
        }
    }
    let (_, base, t) = code.ok_or_else(err)?;
    let (_, end, r) = ro.ok_or_else(err)?;
    if base % 4096 != 0
        || base.checked_add(t.len()) != Some(end)
        || end.checked_add(r.len()).is_none_or(|n| n % 4096 != 0)
    {
        return Err(err());
    }
    let mut image = t.to_vec();
    image.extend_from_slice(r);
    Ok(sha256_hex(&image))
}
struct Temp {
    path: PathBuf,
    guest: &'static str,
}
impl Drop for Temp {
    fn drop(&mut self) {
        for file in ["kernel.elf32", self.guest, "doom1.wad"] {
            let _ = fs::remove_file(self.path.join(file));
        }
        let _ = fs::remove_dir(&self.path);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 4 {
        return Err(
            "Usage: pcp-native-store KERNEL GAME [IWAD | host|peer PORT]".into(),
        );
    }
    let (shelf, file, doom_options) = match args[1].as_str() {
        "gullpost" => ("games/gullpost-express", "gullpost.elf", None),
        "gull64" => ("games/gullpost-express-64", "gull64.elf", None),
        "sunspur" => ("games/sunspur-rally", "sunspur.elf", None),
        "fluxrink" => ("games/flux-rink", "fluxrink.elf", None),
        "heliograph" => ("engines/heliograph", "heliograph.elf", None),
        "doom" => ("games/doom", "doom.elf", Some("")),
        "ultra-omega-barrel" => ("games/ultra-omega-barrel", "omega.elf", Some("-omega")),
        "barrel-bowl" => ("games/barrel-bowl", "bowl.elf", Some("-omega -bowl -warp 1 9")),
        "midnight-barrel-express" => ("games/midnight-barrel-express", "rail.elf", Some("-omega -express -warp 1 9")),
        "forklift-certified" => ("games/forklift-certified", "fork.elf", Some("-omega -forklift -warp 1 9")),
        _ => return Err("Unknown Store guest".into()),
    };
    let link = match &args[2..] {
        [] if doom_options.is_none() => None,
        [_] if doom_options.is_some() => None,
        [role, port] if file == "fluxrink.elf" && matches!(role.as_str(), "host" | "peer") => {
            let port: u16 = port.parse()?;
            if port == 0 {
                return Err("Choose a nonzero loopback port".into());
            }
            Some((role, port))
        }
        _ if doom_options.is_some() => return Err("Supply the path to your local IWAD".into()),
        _ => return Err("Cable mode requires fluxrink, host or peer, and a port".into()),
    };
    let store = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let kernel = fs::read(&args[0])?;
    let measured = measurement(&kernel)?;
    let payload = fs::read(store.join(shelf).join("dist").join(file))?;
    let dir = env::temp_dir().join(format!(
        "pcp-native-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&dir)?;
    let tmp = Temp { path: dir, guest: file };
    fs::write(tmp.path.join("kernel.elf32"), kernel)?;
    fs::write(tmp.path.join(file), &payload)?;
    let mut modules = file.to_owned();
    if let Some(options) = doom_options {
        let wad = tmp.path.join("doom1.wad");
        if args[1] == "doom" {
            fs::copy(&args[2], &wad)?;
        } else {
            let python = env::var_os("PCPOS_PYTHON").unwrap_or_else(|| {
                if cfg!(windows) { "python".into() } else { "python3".into() }
            });
            let status = Command::new(python)
                .arg("-B")
                .arg(store.join("games/doom/prepare.py"))
                .args(&args[1..3])
                .arg(&wad)
                .status()?;
            if !status.success() {
                return Err("Could not prepare the local game data".into());
            }
        }
        let arguments = format!("{options} -iwad doom1.wad");
        modules = format!("{file} {},doom1.wad", arguments.trim_start());
    }
    let exe = env::var_os("PCPOS_QEMU").unwrap_or_else(|| "qemu-system-x86_64".into());
    let mut vm = Command::new(&exe);
    if let Some(share) = Path::new(&exe)
        .parent()
        .map(|p| p.join("share"))
        .filter(|p| p.is_dir())
    {
        vm.arg("-L").arg(share);
    }
    let mut boot = format!("image={measured}");
    if doom_options.is_some() {
        boot += &format!(" auto={file} then=off");
        let driver = match env::var_os("PCPOS_AUDIO") {
            Some(selected) => selected.to_string_lossy().into_owned(),
            None => {
                let help = Command::new(&exe).args(["-audiodev", "help"]).output()?;
                let available = format!("{}\n{}", String::from_utf8_lossy(&help.stdout),
                                        String::from_utf8_lossy(&help.stderr));
                ["dsound", "coreaudio", "pipewire", "pa", "sdl", "alsa"]
                    .into_iter()
                    .find(|driver| available.lines().any(|line| line.trim() == *driver))
                    .unwrap_or("none")
                    .to_owned()
            }
        };
        if !driver.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') || driver.is_empty() {
            return Err("PCPOS_AUDIO must name one QEMU audio backend".into());
        }
        if driver != "none" {
            vm.args(["-audiodev", &format!("{driver},id=doom")]);
            let help = Command::new(&exe).args(["-device", "help"]).output()?;
            let devices = format!("{}\n{}", String::from_utf8_lossy(&help.stdout),
                                  String::from_utf8_lossy(&help.stderr));
            for device in ["AC97", "adlib"] {
                if devices.contains(&format!("\"{device}\"")) {
                    vm.args(["-device", &format!("{device},audiodev=doom")]);
                }
            }
        }
    }
    vm.current_dir(&tmp.path).args([
        "-cpu",
        "max",
        "-m",
        "256M",
        "-vga",
        "std",
        "-global",
        "VGA.vgamem_mb=32",
        "-nic",
        "none",
        "-no-reboot",
        "-kernel",
        "kernel.elf32",
        "-initrd",
        &modules,
        "-serial",
        "stdio",
        "-device",
        "isa-debug-exit,iobase=0xf4,iosize=0x04",
    ]);
    if let Some((role, port)) = link {
        boot += &format!(" link_image={}", sha256_hex(&payload));
        vm.args([
            "-serial",
            &format!(
                "tcp:127.0.0.1:{port}{}",
                if role == "host" {
                    ",server=on,wait=off"
                } else {
                    ""
                }
            ),
        ]);
    }
    vm.arg("-append").arg(boot);
    let status = vm.status()?;
    if status.code() != Some(33) && !(doom_options.is_some() && status.success()) {
        return Err(format!("QEMU exited with {status}").into());
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
