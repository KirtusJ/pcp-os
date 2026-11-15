# Native Doom engine source

The Store guest is the unmodified Doomgeneric engine at `e257a09527062bfb986b6e5e0780839b6cbd5b71` with PCP OS input, display and sound. Its OPL player comes from Chocolate Doom at `a2dfd51fcc00d272dc8a17a97ccd6683333ff4ea`. The source archive contains both projects' C sources used by this build, their headers and notices, the PCP OS platform files and the original kernel build script. It also includes musl 1.2.5 and Alpine's musl package recipe and patches from `ddaf59b84bc6983a37d6e24d2210c2c4791642bb`. [Doomgeneric](https://github.com/ozkl/doomgeneric/tree/e257a09527062bfb986b6e5e0780839b6cbd5b71) · [Chocolate Doom](https://github.com/chocolate-doom/chocolate-doom/tree/a2dfd51fcc00d272dc8a17a97ccd6683333ff4ea) · [Alpine source package](https://github.com/alpinelinux/aports/tree/ddaf59b84bc6983a37d6e24d2210c2c4791642bb/main/musl)

## Reproduce the Store ELF

Install Python 3.9+, Zig 0.14.1, LLD 23.1.5 and llvm-objcopy 23.1.5. Download the Alpine v3.22 x86_64 development package. Its SHA-256 is `2ca496dd68c0b340b50cf9940bc2f3312eac8b56ec2d3284a621be6f0c09ffb1`. Run in a writable directory containing this guide's `engine-source.tar.gz`, `build.py` and the downloaded APK. Supply tool paths through `--zig`, `--lld` and `--objcopy` if needed. [Alpine musl-dev 1.2.5-r13](https://dl-cdn.alpinelinux.org/alpine/v3.22/main/x86_64/musl-dev-1.2.5-r13.apk)

~~~sh
tar xzf engine-source.tar.gz
python3 build.py doom-source musl-dev-1.2.5-r13.apk new-build doom.elf
~~~

The release links 87 objects, musl's static PIE startup object and libc. Two source assertion locations retain their release path spelling in the executable. The builder preserves those strings and the original object order. The expected SHA-256 is `b21e4dc4dd2061cf99660a71d929975d8b362fa9c59eb0b9782deb82c52cecc5`.

## Build with GNU tools on x86-64 Linux

With GCC, GNU binutils, GNU make, Python 3 and a POSIX shell, extract the source archive into a path without spaces. This builds the guest from the included sources using upstream musl 1.2.5. It produces its own executable digest.

~~~sh
cd doom-source
sh build-musl.sh
PATH="$PWD/toolchain/bin:$PATH" sh pcpos-port/build.sh "$PWD/engine/doomgeneric" "$PWD/chocolate-doom" "$PWD/doom.elf"
~~~

The engine is distributed under the GNU GPL. The platform notice identifies the source files offered under GPL-2.0 and the interface header's inclusion terms. Individual upstream copyright notices remain with their sources. [GPL text](GPL-2.0) · [PCP OS platform notice](PCP-OS-NOTICE.md) · [Musl copyright and third-party notices](MUSL-COPYRIGHT) · [Zig copyright notice](Zig-LICENSE)
