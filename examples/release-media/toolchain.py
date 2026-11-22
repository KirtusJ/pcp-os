"""Select and identify the Rust tools used by qualified component builds."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tomllib


def check_external_config(directory):
    # Cargo accepts both spellings. Look at both so changing the preferred
    # spelling cannot silently change the effective profile or compiler.
    for filename in ("config", "config.toml"):
        path = directory / filename
        if path.is_file():
            with path.open("rb") as stream:
                config = tomllib.load(stream)
            extra = sorted(config.keys() - {"source", "registries", "registry", "net", "http", "term", "cache", "build"})
            extra.extend("build." + key for key in sorted(config.get("build", {}).keys() - {"jobs"}))
            if extra:
                raise ValueError(f"external Cargo config {path} sets {', '.join(extra)}; use a dependency-only config for this build")


def build_environment(work):
    work = work.resolve()
    home = Path(os.environ.get("CARGO_HOME") or (Path.home() / ".cargo")).resolve()
    for directory in {home, *(parent / ".cargo" for parent in (work, *work.parents))}:
        check_external_config(directory)

    # Keep dependency-cache, network and offline settings. Set the paths,
    # profile inputs and target flags from the build and its pinned source.
    prefixes = ("RUSTC_", "CARGO_BUILD_", "CARGO_PROFILE_", "CARGO_TARGET_",
                "CARGO_ALIAS_", "CARGO_UNSTABLE_", "CARGO_ENCODED_")
    overrides = {"CARGO", "RUSTC", "RUSTFLAGS", "CARGO_INCREMENTAL", "CARGO_MAKEFLAGS",
                 "RUSTDOCFLAGS", "CARGO_CACHE_RUSTC_INFO"}
    env = os.environ.copy()
    for name in os.environ:
        if name != "CARGO_BUILD_JOBS" and (name.startswith(prefixes) or name in overrides):
            del env[name]
    driver = shutil.which("rustc", path=env.get("PATH"))
    if driver is None:
        raise ValueError("rustc is missing from PATH")
    sysroot = Path(subprocess.check_output([driver, "--print", "sysroot"], cwd=work, env=env, text=True).strip())
    bindir = sysroot.resolve() / "bin"
    for key in ("CARGO", "RUSTC"):
        tool = bindir / (key.lower() + (".exe" if os.name == "nt" else ""))
        if not tool.is_file() or not os.access(tool, os.X_OK):
            raise ValueError(f"the selected Rust toolchain is missing {tool.name}")
        env[key] = str(tool)
    # Leave wrappers absent. Cargo can give an explicitly empty workspace wrapper
    # a different crate identity, even when it executes no wrapper.
    env.update(CARGO_HOME=str(home),
               CARGO_TARGET_DIR=str(work / "target"), TZ="UTC", LC_ALL="C", SOURCE_DATE_EPOCH="1794614400")
    return env


def check_project_wrappers(directory, work):
    directory, work = directory.resolve(), work.resolve()
    for parent in (directory, *directory.parents):
        if not parent.is_relative_to(work):
            break
        for name in ("config", "config.toml"):
            path = parent / ".cargo" / name
            if path.is_file():
                with path.open("rb") as stream:
                    config = tomllib.load(stream)
                wrappers = {"rustc-wrapper", "rustc-workspace-wrapper"} & config.get("build", {}).keys()
                if wrappers or "include" in config:
                    raise ValueError(f"{path} selects a compiler wrapper or includes external configuration")


def tool_record(env, profile="release"):
    record = {"profile": profile}
    for tool in ("cargo", "rustc"):
        path = Path(env[tool.upper()])
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        version = subprocess.check_output([path, "-vV"], env=env, text=True).strip()
        record[tool] = {"version": version, "sha256": digest}
    return record
