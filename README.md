# Proton App Launcher

A tiny, dependency-free Win32 launcher for Wine/Proton prefixes. It shows a
native window with one radio button per application found in
`C:\proton_shortcuts`, launches the selected executable (with optional
arguments, working directory and environment variables), and exits immediately.

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
   ```

3. Run `launcher.exe` once (for example through Proton). On first start it
   scans the shortcuts folder, writes `config.yaml` into it, and shows the
   application list.
4. Select an entry and press **Launch** (or `Enter`). `Escape` or **Cancel**
   closes the launcher.

If `config.yaml` already exists and lists at least one app, `.desktop` files
are ignored. An empty `config.yaml` (no apps) is regenerated from the
`.desktop` files, so adding shortcuts later just needs another run. Edit the
YAML to customise arguments, CWD and environment variables:

```yaml
apps:
  - name: "Game One"
    path: "C:\\Games\\Game1\\game.exe"
    cwd: "C:\\Games\\Game1"
    args:
      - "-dx11"
    env:
      DXVK_HUD: "fps"
```

Delete `config.yaml` to force a full rescan of the `.desktop` files.

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

- No third-party crates: only `std` and raw `user32`/`kernel32` FFI.
- The binary is built with `#![windows_subsystem = "windows"]`, so no console
  window flashes under Wine.
- Configuration parsing supports only the schema above (comments, blank lines,
  quoted/unquoted values and both list indentation styles are tolerated).
