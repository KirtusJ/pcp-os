import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from toolchain import build_environment, check_external_config, check_project_wrappers, tool_record


class QualifiedRust(unittest.TestCase):
    def test_compiler_and_profile_come_from_the_selected_build(self):
        inherited = {
            "RUSTC": "unused-rustc", "RUSTC_WRAPPER": "unused-wrapper",
            "RUSTC_WORKSPACE_WRAPPER": "unused-wrapper", "CARGO_BUILD_RUSTC": "unused-rustc",
            "CARGO_BUILD_RUSTC_WRAPPER": "unused-wrapper",
            "CARGO_PROFILE_RELEASE_OPT_LEVEL": "0", "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS": "false",
            "CARGO_TARGET_X86_64_UNKNOWN_NONE_RUSTFLAGS": "-Copt-level=0",
        }
        with tempfile.TemporaryDirectory() as temp, patch.dict(os.environ, inherited):
            env = build_environment(Path(temp))
            self.assertTrue(Path(env["RUSTC"]).is_absolute())
            self.assertTrue(Path(env["CARGO"]).is_absolute())
            self.assertNotIn("RUSTC_WRAPPER", env)
            self.assertNotIn("RUSTC_WORKSPACE_WRAPPER", env)
            for name in inherited:
                self.assertNotEqual(env.get(name), inherited[name])
            tools = tool_record(env)
            for name in ("rustc", "cargo"):
                self.assertIn("release:", tools[name]["version"])
                self.assertRegex(tools[name]["sha256"], r"^[0-9a-f]{64}$")

    def test_checked_in_target_flags_stay_usable_without_a_compiler_wrapper(self):
        with tempfile.TemporaryDirectory() as temp:
            work = Path(temp)
            directory = work / "source"
            config = directory / ".cargo" / "config.toml"
            config.parent.mkdir(parents=True)
            config.write_text('[target.x86_64-unknown-none]\nrustflags = ["-Copt-level=2"]\n')
            check_project_wrappers(directory, work)
            config.write_text('[build]\nrustc-wrapper = "unused-wrapper"\n')
            with self.assertRaisesRegex(ValueError, "wrapper"):
                check_project_wrappers(directory, work)

    def test_external_sources_are_supported_and_external_profiles_are_reported(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            for name in ("config.toml", "config"):
                with self.subTest(name=name):
                    p = directory / name
                    p.write_text('[build]\njobs=2\n[term]\ncolor="auto"\n[net]\noffline = true\n[source.crates-io]\nreplace-with = "vendor"\n')
                    check_external_config(directory)
                    p.write_text('[profile.release]\nopt-level = 0\n')
                    with self.assertRaisesRegex(ValueError, "profile"):
                        check_external_config(directory)
                    p.unlink()


if __name__ == "__main__":
    unittest.main()
