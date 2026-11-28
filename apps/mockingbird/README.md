# Mockingbird

A Rust text editor for the x86-64 PCP OS guest. Its document lives in RAM. In the Windows VM package, Ctrl+V pastes plain text from Windows and Ctrl+X cuts selected text to Windows. Ctrl+C copies. Use Shift with the arrows to select text, or Ctrl+A to select the document. Esc opens a discard prompt when the document contains text.

The document holds up to 32 KiB of UTF-8. Unsupported glyphs display as a question mark and retain their original text when copied. Notes and Find queries use the kernel's DejaVu Sans Mono. The wordmark and interface labels use Source Sans 3, rasterized for the guest at both display scales. A running instance can edit with an internal clipboard on a kernel without the optional guest cable. [Source Sans 3 artwork and license](https://github.com/adobe-fonts/source-sans/tree/ed1808970eb3c7301c9a523bee26473ba0bb62fa) · [Bundled notice](brand/Source-Sans-LICENSE.txt)

## Run version 0.3.1 on Windows

In 0.3.1, pressing Enter at the right edge starts a single new row. Moving vertically across a shorter line restores the original column when space permits. The wordmark and interface labels use proportional lettering in both appearances.

1. Download the [portable VM ZIP](../../store/apps/mockingbird/dist/mockingbird-0.3.1-windows-vm.zip). On the GitHub file page, choose **Download raw file**. The [Store entry](../../store/index/kirtusj/mo/ck/mockingbird) records the ZIP size, SHA-256 and immutable payload commit. A clone also includes the ZIP.
2. Extract the entire ZIP into a new folder. The host needs Windows Python 3.11 or newer and x86-64 QEMU with its GTK display and bundled OVMF firmware. Put QEMU on PATH, install it under `Program Files\qemu`, or set `PCPOS_QEMU` to the executable.
3. Open **Open Mockingbird.cmd**. The VM boots straight into the editor with a blank, light sheet. Press **F6** for dark. Closing the guest discards the note.

The [standalone x86-64 ELF](../../store/apps/mockingbird/dist/mockingbird.elf) is the same executable carried inside the ZIP. The VM supplies the measured kernel and clipboard bridge needed for Windows clipboard exchange. To replay the synthetic clipboard and theme checks from a source checkout, run `python apps/mockingbird/verify_guest.py "C:/path/to/extracted-package"` from the repository root.

Keep the [Source Sans license notice](brand/Source-Sans-LICENSE.txt) with the standalone ELF. The portable ZIP includes the same notice under `media/PCPOS/`.

## Editing a scratch note

| Action | Keys |
| --- | --- |
| Undo a typing run, deletion, cut or paste | Ctrl+Z |
| Redo | Ctrl+Shift+Z or Ctrl+Y |
| Find literal text | Ctrl+F |
| Next or previous match, wrapping at the ends | Enter or Shift+Enter |
| Close Find and edit the selected match | Esc |
| Repeat the last search from the sheet | F3 or Shift+F3 |

Undo keeps the last twelve edits in the guest. Consecutive typing groups until a pause of about a second or a navigation command. Cut and paste each form their own step. Undoing a cut restores the text and its selection in the document. The clipboard still holds what you last copied.

Find uses exact, case-sensitive text. A single-line selection of up to 256 UTF-8 bytes becomes the initial query. You can type over it or paste a query. The strip closes back onto the current match.

Mockingbird opens directly onto one plain-text sheet. **Session only** tells you what will happen to the note when the VM closes. The long-tailed bird in the header and the Windows launcher icon come from the same [vector artwork](brand/mockingbird.svg).

## Appearance

Every Mockingbird session starts in light appearance. Press **F6** to switch to dark, and press it again to switch back. The header names the appearance F6 will select. You can switch while typing, selecting text or using Find.

| Role | Light | Dark |
| --- | --- | --- |
| Sheet and Find field | `#dad6cb` | `#1c1c1c` |
| Document text and caret | `#1c1c1c` | `#dad6cb` |
| Header and footer | `#dce2e0` | `#1c1c1c` |
| App name and bird | `#1c1c1c` | `#dce2e0` |
| Session label and shortcut hints | `#1c1c1c` | `#b9b5b5` |
| Header and footer dividers | `#b9b5b5` | `#7f7e7e` |
| Find field underline | `#7f7e7e` | `#7f7e7e` |
| Selection fill and lettering | `#1c1c1c` / `#dad6cb` | `#b9b5b5` / `#1c1c1c` |

The guest renderer applies one palette to the whole editor, including the bird, the caret and selected text in Find. The appearance belongs to the running Mockingbird window.

## Design choices

People who use an editor for temporary notes described both the convenience of restored tabs and the confusion of finding old notes on launch. Mockingbird follows the single-sheet workflow requested for PCP OS and makes its lifetime visible in the header. [Community discussion](https://www.reddit.com/r/msp/comments/1btpcpb/anybody_else_hate_the_new_notepad_in_windows/)

Multi-level undo was a frequently requested Notepad improvement. In Mockingbird it applies to the clipboard workflow too, so one undo restores a cut selection. [Microsoft's Notepad design note](https://blogs.windows.com/windows-insider/2021/12/07/redesigned-notepad-for-windows-11-begins-rolling-out-to-windows-insiders/)

GNOME Text Editor and macOS TextEdit both support starting a search from selected text. Mockingbird brings that selection into a temporary Find strip. Vim's repeat-search commands inspired F3 and Shift+F3 on the sheet. [GNOME Text Editor](https://help.gnome.org/gnome-text-editor/edit-search-and-replace.html) · [Apple TextEdit](https://support.apple.com/guide/textedit/find-and-replace-text-txtef6cfde1a/mac) · [Vim user manual](https://vimhelp.org/usr_03.txt.html#03.8)

## Build the Windows VM package

From this directory, with Python 3.11 or newer, pycdlib and the Windows GNU Rust toolchain including x86_64-unknown-none, run the following with a new output directory.

~~~powershell
python package.py D:/path/to/Mockingbird --toolchain D:/path/to/rust-toolchain
~~~

Open **Mockingbird.lnk** in the output directory. The shortcut uses the Python installation and package location from build time. **Open Mockingbird.cmd** also works after moving the directory. The launcher finds x86-64 QEMU on PATH or uses PCPOS_QEMU, which can point to the executable. It expects the unsigned x86-64 OVMF code and variables in QEMU's share directory.

Add `--archive` to the build command to write the portable ZIP in the output directory. It includes the relative launcher, icon, boot files and notices. The Windows shortcut is specific to the build machine.

The package uses the daily driver's model-free assets and builds the PCP OS kernel with its existing proposals and game-link features. The bootstrap grants the clipboard cable to the exact editor ELF digest. The local Windows helper exchanges plain text on explicit clipboard commands, using the emulated COM2 serial port. The VM has no virtual network card. The launcher gives each invocation its own firmware variables and attaches the boot files read-only.
