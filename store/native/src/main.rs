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
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("kernel.elf32"));
        for file in ["fluxrink.elf", "sunspur.elf", "heliograph.elf"] {
            let _ = fs::remove_file(self.0.join(file));
        }
        let _ = fs::remove_dir(&self.0);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 4 {
        return Err(
            "Usage: pcp-native-store KERNEL {sunspur|heliograph|fluxrink} [host|peer PORT]".into(),
        );
    }
    let (shelf, file) = match args[1].as_str() {
        "sunspur" => ("games/sunspur-rally", "sunspur.elf"),
        "fluxrink" => ("games/flux-rink", "fluxrink.elf"),
        "heliograph" => ("engines/heliograph", "heliograph.elf"),
        _ => return Err("Unknown Store guest".into()),
    };
    let link = match &args[2..] {
        [] => None,
        [role, port] if file == "fluxrink.elf" && matches!(role.as_str(), "host" | "peer") => {
            let port: u16 = port.parse()?;
            if port == 0 {
                return Err("Choose a nonzero loopback port".into());
            }
            Some((role, port))
        }
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
    let tmp = Temp(dir);
    fs::write(tmp.0.join("kernel.elf32"), kernel)?;
    fs::write(tmp.0.join(file), &payload)?;
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
    vm.current_dir(&tmp.0).args([
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
        file,
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
    if status.code() != Some(33) {
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
