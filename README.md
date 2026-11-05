# Proton App Launcher

A tiny, dependency-free Win32 launcher for Wine/Proton prefixes. It shows a
native window with one radio button per application found in
`C:\proton_shortcuts`, launches the selected executable (with optional
arguments, working directory and environment variables), and shows each app's
icon when one can be found. A built-in **Terminal** entry
(`C:\Windows\Command\start.exe`) is always available. After a successful
launch the launcher stays open, so you can start another entry; **Escape** or
**Cancel** closes it.

## Usage

1. Put `launcher.exe` into `C:\proton_shortcuts` (i.e.
   `~/.steam/steam/steamapps/compatdata/<appid>/pfx/drive_c/proton_shortcuts`)
   or anywhere else in the prefix.
2. Add `*.desktop` files there, for example:

   ```ini
   [Desktop Entry]
   Name=Game One
   Exec="C:\\Games\\Game1\\game.exe" -dx11
   Path=C:\\Games\\Game1
   Icon=Game.0
   ```

3. Run `launcher.exe` once (for example through Proton). On first start it
   scans the shortcuts folder, writes `config.yaml` into it, and shows the
   application list.
4. Select an entry and press **Launch** (or `Enter`). The window stays open;
   `Escape` or **Cancel** closes the launcher.

If `config.yaml` already exists and lists at least one app, `.desktop` files
are ignored. An empty `config.yaml` (no apps) is regenerated from the
`.desktop` files, so adding shortcuts later just needs another run; a
non-empty file that cannot be parsed is left untouched. The launcher re-reads
`config.yaml` before every launch, so editing arguments, CWD or environment
variables takes effect without restarting (adding/removing apps needs a
restart because the radio list is built at startup). Edit the YAML to
customise arguments, CWD and environment variables:

```yaml
apps:
  - name: "Game One"
    path: "C:\\Games\\Game1\\game.exe"
    icon: "Game.0"
    cwd: "C:\\Games\\Game1"
    args:
      - "-dx11"
    env:
      DXVK_HUD: "fps"
```

`args` is normally a list, one argument per `- "..."` line. For convenience an
inline list (`args: ["-dx11", "-novid"]`) is accepted too, and a single value
on the `args:` line (`args: -dx11 -novid`) is split like a Windows command
line. `#` starts a comment outside quotes. Config files saved as UTF-8
(optionally with BOM) or UTF-16 with a BOM (as Notepad writes) are read.

`icon` is optional. A bare name is looked up in
`icons/<size>/apps/<Icon>.png` (`.ico` and extensionless files are also
tried); the launcher chooses the size closest to 32x32, preferring the smallest
size at least 32. A Windows/Linux path in `icon` is used directly. Missing or
unloadable icons are ignored, so the list still works without icon files.
Existing configs without an `icon` key get icons in memory from a matching
`.desktop` `Icon=` entry (path first, then app name); the file itself is not
rewritten.

The **Terminal** entry is appended in memory after the configured apps and is
never written into a generated `config.yaml`. Add an app with path
`C:\Windows\Command\start.exe` yourself to customise or replace it.

`Exec=` may point at a Windows `.lnk` shortcut. The launcher resolves it with
`IShellLinkW`, appends the shortcut's own parameters, and starts the real target
with `args`/`cwd`/`env` from the config; if resolution fails it falls back to
`explorer.exe`. Wine-style `Path=` values (`.../dosdevices/c:/Program Files/...`)
are rewritten to `C:\Program Files\...`.

Proton writes a shortcut's argument string as one quoted token in the generated
`.desktop` file (`Exec="C:\\...\\game.exe" "-mode 1 -log foo"`). The launcher
splits that token with Windows command-line rules, so the target receives the
same individual arguments as it would from the `.lnk` itself.

If a `.lnk` cannot be resolved, the launcher opens it through `explorer.exe`;
arguments from `config.yaml` cannot be forwarded in that fallback.

Delete `config.yaml` to force a full rescan of the `.desktop` files. (Do this
once after upgrading from a version that mangled `.lnk` paths or passed the
whole shortcut argument string as a single argument.)

## Build

Cross-compile on Linux:

```sh
rustup target add x86_64-pc-windows-gnu
sudo apt-get install gcc-mingw-w64-x86-64
cargo build --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/launcher.exe .
```

CI builds `launcher.exe` and overwrites the rolling `latest` release on every
push, so this link always downloads the newest build:

<https://github.com/dimhara/proton_metalauncher/releases/latest/download/launcher.exe>

The matching checksum is `launcher.exe.sha256` next to it.

## Notes

- No third-party crates: only `std` and raw `user32`/`kernel32` FFI. `ole32`
  (`.lnk` resolution) and `gdiplus` (PNG icon rendering) are loaded on demand
  at runtime, so they are not in the executable's import table.
- The binary is built with `#![windows_subsystem = "windows"]`, so no console
  window flashes under Wine.
- Configuration parsing supports only the schema above (comments, blank lines,
  quoted/unquoted values and both list indentation styles are tolerated).
