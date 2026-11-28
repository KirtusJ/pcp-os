# Mockingbird

A Rust text editor for the x86-64 PCP OS guest. Its document lives in RAM. In the Windows VM package, Ctrl+V pastes plain text from Windows and Ctrl+X cuts selected text to Windows. Ctrl+C copies. Use Shift with the arrows to select text, or Ctrl+A to select the document. Esc opens a discard prompt when the document contains text.

The document holds up to 32 KiB of UTF-8. Unsupported glyphs display as a question mark and retain their original text when copied. The editor uses the kernel's existing DejaVu Sans Mono glyphs. A running instance can edit with an internal clipboard on a kernel without the optional guest cable.

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

Mockingbird opens directly onto one plain-text sheet. The small bird with a pale wing bar marks the application, and **Session only** tells you what will happen to the note when the VM closes.

## Design choices

People who use an editor for temporary notes described both the convenience of restored tabs and the confusion of finding old notes on launch. Mockingbird follows the single-sheet workflow requested for PCP OS and makes its lifetime visible in the header. [Community discussion](https://www.reddit.com/r/msp/comments/1btpcpb/anybody_else_hate_the_new_notepad_in_windows/)

Multi-level undo was a frequently requested Notepad improvement. In Mockingbird it applies to the clipboard workflow too, so one undo restores a cut selection. [Microsoft's Notepad design note](https://blogs.windows.com/windows-insider/2021/12/07/redesigned-notepad-for-windows-11-begins-rolling-out-to-windows-insiders/)

GNOME Text Editor and macOS TextEdit both support starting a search from selected text. Mockingbird brings that selection into a temporary Find strip. Vim's repeat-search commands inspired F3 and Shift+F3 on the sheet. [GNOME Text Editor](https://help.gnome.org/gnome-text-editor/edit-search-and-replace.html) · [Apple TextEdit](https://support.apple.com/guide/textedit/find-and-replace-text-txtef6cfde1a/mac) · [Vim user manual](https://vimhelp.org/usr_03.txt.html#03.8)

## Build the Windows VM package

From this directory, with Python 3.11 or newer, pycdlib and the Windows GNU Rust toolchain including x86_64-unknown-none, run the following with a new output directory.

~~~powershell
python package.py D:/path/to/Mockingbird --toolchain D:/path/to/rust-toolchain
~~~

Open **Open Mockingbird.cmd** in the output directory. The launcher finds x86-64 QEMU on PATH or uses PCPOS_QEMU, which can point to the executable. It expects the unsigned x86-64 OVMF code and variables in QEMU's share directory.

The package uses the daily driver's model-free assets and builds the PCP OS kernel with its existing proposals and game-link features. The bootstrap grants the clipboard cable to the exact editor ELF digest. The local Windows helper exchanges plain text on explicit clipboard commands, using the emulated COM2 serial port. The VM has no virtual network card. The launcher gives each invocation its own firmware variables and attaches the boot files read-only.
