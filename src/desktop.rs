//! Minimal freedesktop `.desktop` parser: extracts `Name=`, `Exec=`, `Path=` and
//! `Icon=` from the `[Desktop Entry]` group.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub path: Option<String>,
    pub args: Vec<String>,
    pub icon: Option<String>,
}

/// Returns all `*.desktop` entries in `dir`, sorted by file name. Unreadable or
/// unusable files are skipped.
pub fn scan_desktop_files(dir: &Path) -> Vec<DesktopEntry> {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_desktop_extension(path))
        .collect();
    files.sort();

    files
        .iter()
        .filter_map(|path| parse_desktop_file(path))
        .collect()
}

fn has_desktop_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("desktop"))
        .unwrap_or(false)
}

fn parse_desktop_file(path: &Path) -> Option<DesktopEntry> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let fallback_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Application")
        .to_string();

    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut cwd: Option<String> = None;
    let mut icon: Option<String> = None;
    let mut in_entry = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "Name" if name.is_none() => name = Some(unescape(value)),
            // Keep `Exec=` raw: `split_exec` owns its escape handling.
            "Exec" if exec.is_none() => exec = Some(value.to_string()),
            "Path" if cwd.is_none() => cwd = Some(unescape(value)),
            "Icon" if icon.is_none() => icon = Some(unescape(value)),
            _ => {}
        }
    }

    let mut parts = split_exec(&exec?);
    if parts.is_empty() {
        return None;
    }
    let program = parts.remove(0);

    Some(DesktopEntry {
        name: name.unwrap_or(fallback_name),
        exec: program,
        // `Path=` is often a host-side Linux path; keep it only when it can
        // be expressed as a Windows directory.
        path: cwd.and_then(|value| windows_cwd(&value)),
        args: parts,
        icon,
    })
}

/// Returns candidate icon files for `icon`, in the order the loader should try
/// them. Files are not checked here; the caller loads candidates until one
/// succeeds.
///
/// Bare names are looked up under `<dir>/icons/<N>x<N>/apps/`, preferring the
/// smallest size at least 32 pixels, then larger sizes and finally smaller
/// sizes.
pub fn icon_candidates(dir: &Path, icon: &str) -> Vec<PathBuf> {
    let icon = icon.trim();
    if icon.is_empty() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let direct = is_windows_path(icon) || icon.starts_with('/') || icon.contains(['\\', '/']);
    if direct {
        let path = PathBuf::from(icon);
        push_unique(&mut candidates, path.clone());
        if !has_image_extension(icon) {
            push_unique(&mut candidates, append_suffix(&path, ".png"));
            push_unique(&mut candidates, append_suffix(&path, ".ico"));
        }
        return candidates;
    }

    let icons_dir = dir.join("icons");
    let Ok(read_dir) = fs::read_dir(&icons_dir) else {
        return candidates;
    };

    let mut sizes: Vec<u32> = read_dir
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| parse_size_dir(&entry.file_name().to_string_lossy()))
        .collect();
    sizes.sort_unstable();
    sizes.dedup();

    // The first entry >= 32 is the smallest such size; then any larger sizes,
    // then smaller sizes from largest to smallest.
    let (mut larger, smaller): (Vec<u32>, Vec<u32>) =
        sizes.into_iter().partition(|&size| size >= 32);
    larger.extend(smaller.into_iter().rev());

    let has_extension = has_image_extension(icon);
    for size in larger {
        let apps = icons_dir.join(format!("{size}x{size}")).join("apps");
        if has_extension {
            push_unique(&mut candidates, apps.join(icon));
            if let Some(stem) = Path::new(icon).file_stem() {
                push_unique(&mut candidates, apps.join(stem));
            }
        } else {
            push_unique(&mut candidates, apps.join(format!("{icon}.png")));
            push_unique(&mut candidates, apps.join(format!("{icon}.ico")));
            push_unique(&mut candidates, apps.join(icon));
        }
    }
    candidates
}

fn parse_size_dir(name: &str) -> Option<u32> {
    let (width, height) = name.split_once(['x', 'X'])?;
    let width: u32 = width.parse().ok()?;
    let height: u32 = height.parse().ok()?;
    (width == height && width > 0).then_some(width)
}

fn has_image_extension(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            ext.eq_ignore_ascii_case("png")
                || ext.eq_ignore_ascii_case("ico")
                || ext.eq_ignore_ascii_case("bmp")
        })
        .unwrap_or(false)
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn push_unique(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

/// Handles the string escapes allowed in desktop-entry values.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Splits an `Exec=` value into program and arguments, respecting quotes and
/// dropping `%`-field codes.
///
/// The value is unescaped first and then split according to the `Exec`
/// quoting rules, as the specification requires. That also handles the files
/// Proton tooling generates, which are not strictly spec-compliant: Windows
/// paths are double-escaped (`C:\\\\Games`) and spaces may be escaped as `\ `
/// rather than `\s`.
fn split_exec(exec: &str) -> Vec<String> {
    let value = unescape(exec);
    let mut parts = split_exec_tokens(&value);

    // winemenubuilder writes a shortcut's whole argument string as one quoted
    // token: `Exec="C:\\...\\game.exe" "-mode 1 -log ..."`. Those arguments
    // have to be split like the shortcut's own command line, otherwise the
    // target receives them as a single argument.
    // winemenubuilder always quotes the program too (`Exec="C:\\...\\game.exe"
    // "-mode 1"`), which distinguishes it from a normal quoted single
    // argument such as `Exec=game "one argument with spaces"`.
    if parts.len() == 2 && parts[0].1 && parts[1].1 && is_windows_path(&parts[0].0) {
        let args = split_windows_args(&parts[1].0);
        parts.truncate(1);
        parts.extend(args.into_iter().map(|arg| (arg, false)));
    }

    parts
        .into_iter()
        .map(|(part, _)| part)
        .filter(|part| !part.starts_with('%'))
        .collect()
}

/// Tokenizes an already unescaped `Exec=` value. The boolean marks tokens that
/// started with a quote; the winemenubuilder compatibility path needs it.
fn split_exec_tokens(value: &str) -> Vec<(String, bool)> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quoted = false;
    let mut quote: Option<char> = None;
    let mut chars = value.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) if c == '\\' => {
                push_exec_escape(&mut current, &mut chars);
                started = true;
            }
            Some(_) => {
                current.push(c);
                started = true;
            }
            None if c == '"' || c == '\'' => {
                if !started {
                    quoted = true;
                }
                quote = Some(c);
                started = true;
            }
            None if c == '\\' => {
                push_exec_escape(&mut current, &mut chars);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    parts.push((std::mem::take(&mut current), quoted));
                    started = false;
                    quoted = false;
                }
            }
            None => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        parts.push((current, quoted));
    }
    parts
}

/// Applies the `Exec` quoting escape rules. A backslash before whitespace is
/// accepted as an escaped space, as written by Proton tooling.
fn push_exec_escape(out: &mut String, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek().copied() {
        Some(next) if next.is_whitespace() => {
            out.push(' ');
            chars.next();
        }
        Some(next) if matches!(next, '"' | '\'' | '`' | '$' | '\\') => {
            out.push(next);
            chars.next();
        }
        // Not a quoting escape (for example a hand-written `C:\Games`): keep
        // the backslash and let the next character be processed normally.
        _ => out.push('\\'),
    }
}

/// Splits a Windows command line with the `CommandLineToArgvW` rules, so an
/// argument string taken from a shortcut reaches the target unchanged. Also
/// used for a scalar `args:` value in `config.yaml`.
pub(crate) fn split_windows_args(command_line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut started = false;
    let mut chars = command_line.chars().peekable();

    while let Some(c) = chars.next() {
        if !in_quotes && c.is_whitespace() {
            if started {
                args.push(std::mem::take(&mut current));
                started = false;
            }
            continue;
        }
        started = true;
        match c {
            '"' => in_quotes = !in_quotes,
            '\\' => {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                if chars.peek() == Some(&'"') {
                    // 2n backslashes leave n backslashes; an odd one also
                    // escapes the quote instead of ending the argument.
                    for _ in 0..count / 2 {
                        current.push('\\');
                    }
                    if count % 2 == 1 {
                        current.push('"');
                        chars.next();
                    }
                } else {
                    for _ in 0..count {
                        current.push('\\');
                    }
                }
            }
            _ => current.push(c),
        }
    }
    if started {
        args.push(current);
    }
    args
}

/// True for `C:\...`, `C:/...` and `\\server\share` paths.
pub fn is_windows_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/'))
        || value.starts_with("\\\\")
}

/// Collapses repeated separators and converts `/` to `\`, keeping a leading
/// UNC prefix.
pub fn normalize_windows_path(value: &str) -> String {
    if !is_windows_path(value) {
        return value.to_string();
    }
    let unc = value.starts_with("\\\\");
    let body = if unc { &value[2..] } else { value };
    let mut out = String::with_capacity(value.len());
    if unc {
        out.push_str("\\\\");
    }
    let mut previous_sep = unc;
    for c in body.chars() {
        let c = if c == '/' { '\\' } else { c };
        if c == '\\' {
            if !previous_sep {
                out.push(c);
            }
            previous_sep = true;
        } else {
            out.push(c);
            previous_sep = false;
        }
    }
    out
}

/// Converts a `Path=` value to a Windows directory, or returns `None` when it
/// cannot be expressed as one. Wine prefixes expose host paths as
/// `<prefix>/dosdevices/c:/Program Files/...`, which becomes
/// `C:\Program Files\...`.
pub fn windows_cwd(value: &str) -> Option<String> {
    let value = value.trim();
    if is_windows_path(value) {
        return Some(normalize_windows_path(value));
    }
    let marker = "/dosdevices/";
    let rest = &value[value.find(marker)? + marker.len()..];
    let (drive, tail) = rest.split_once(':')?;
    let mut letters = drive.chars();
    let letter = letters.next()?;
    if !letter.is_ascii_alphabetic() || letters.next().is_some() {
        return None;
    }
    let tail: String = tail
        .trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| if c == '/' { '\\' } else { c })
        .collect();
    Some(format!("{}:\\{}", letter.to_ascii_uppercase(), tail))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("metalauncher-desktop-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn splits_quoted_exec_and_strips_field_codes() {
        let parts = split_exec("\"/opt/My Game/run.sh\" --foo 'bar baz' %U --x=%f");
        assert_eq!(
            parts,
            vec!["/opt/My Game/run.sh", "--foo", "bar baz", "--x=%f"]
        );
    }

    #[test]
    fn splits_winemenubuilder_argument_blob() {
        let dir = temp_dir("lnk-args");
        let file = dir.join("game.desktop");
        // Exact shape winemenubuilder writes: the shortcut's whole argument
        // string is a single quoted token.
        fs::write(
            &file,
            r#"[Desktop Entry]
Name=My Game
Exec="C:\\\\Games\\\\game.exe" "-mode 1 -log \\"my log.txt\\""
Type=Application
"#,
        )
        .unwrap();

        let entry = parse_desktop_file(&file).unwrap();
        assert_eq!(entry.exec, r"C:\Games\game.exe");
        assert_eq!(entry.args, vec!["-mode", "1", "-log", "my log.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_winemenubuilder_args_add_no_argument() {
        let dir = temp_dir("lnk-noargs");
        let file = dir.join("game.desktop");
        fs::write(
            &file,
            "[Desktop Entry]\nName=Game\nExec=\"C:\\\\Games\\\\game.exe\" \"\"\n",
        )
        .unwrap();

        let entry = parse_desktop_file(&file).unwrap();
        assert_eq!(entry.exec, r"C:\Games\game.exe");
        assert!(entry.args.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn separate_quoted_arguments_stay_separate() {
        let parts = split_exec(r#""C:\\Games\\game.exe" "-foo" "-bar baz""#);
        assert_eq!(parts, vec![r"C:\Games\game.exe", "-foo", "-bar baz"]);
    }

    #[test]
    fn single_quoted_argument_with_spaces_is_not_resplit() {
        // A normal Exec line with one quoted argument must not be mistaken
        // for winemenubuilder's flattened shortcut arguments.
        let parts = split_exec(r#"game "one argument with spaces""#);
        assert_eq!(parts, vec!["game", "one argument with spaces"]);

        // Same when the program itself is quoted but is not a Windows path.
        let parts = split_exec(r#""/opt/My Game/game" "one argument with spaces""#);
        assert_eq!(parts, vec!["/opt/My Game/game", "one argument with spaces"]);
    }

    #[test]
    fn parses_file_and_ignores_other_groups() {
        let dir = temp_dir("parse");
        let file = dir.join("Game.desktop");
        let mut f = fs::File::create(&file).unwrap();
        writeln!(f, "[Desktop Entry]").unwrap();
        writeln!(f, "Name=My Game").unwrap();
        writeln!(f, "Exec=\"C:\\\\Games\\\\game.exe\" -dx11 %U").unwrap();
        writeln!(f, "Path=C:\\\\Games").unwrap();
        writeln!(f, "[Desktop Action other]").unwrap();
        writeln!(f, "Name=Wrong").unwrap();

        let entry = parse_desktop_file(&file).unwrap();
        assert_eq!(entry.name, "My Game");
        assert_eq!(entry.exec, r"C:\Games\game.exe");
        assert_eq!(entry.path.as_deref(), Some(r"C:\Games"));
        assert_eq!(entry.args, vec!["-dx11"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_double_escaped_shortcut_path() {
        let dir = temp_dir("lnk-quoted");
        let file = dir.join("7zip.desktop");
        fs::write(
            &file,
            r#"[Desktop Entry]
Name=7-Zip File Manager
Exec="C:\\\\ProgramData\\\\Microsoft\\\\Windows\\\\Start Menu\\\\Programs\\\\7-Zip\\\\7-Zip File Manager.lnk"
Type=Application
"#,
        )
        .unwrap();

        let entry = parse_desktop_file(&file).unwrap();
        assert_eq!(
            entry.exec,
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\7-Zip\7-Zip File Manager.lnk"
        );
        assert!(entry.args.is_empty());
        assert_eq!(entry.path, None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn escaped_space_and_dosdevices_cwd_are_normalized() {
        let dir = temp_dir("lnk-space");
        let file = dir.join("firefox.desktop");
        fs::write(
            &file,
            r#"[Desktop Entry]
Name=Firefox
Exec=C:\\\\ProgramData\\\\Microsoft\\\\Windows\\\\Start\ Menu\\\\Programs\\\\Firefox.lnk
Path=/home/dosemu/wine/vortex/pfx/dosdevices/c:/Program Files/Mozilla Firefox
Type=Application
"#,
        )
        .unwrap();

        let entry = parse_desktop_file(&file).unwrap();
        assert_eq!(
            entry.exec,
            r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Firefox.lnk"
        );
        assert_eq!(
            entry.path.as_deref(),
            Some(r"C:\Program Files\Mozilla Firefox")
        );
        assert!(entry.args.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_skips_unusable_files_and_sorts() {
        let dir = temp_dir("scan");
        fs::write(
            dir.join("b.desktop"),
            "[Desktop Entry]\nName=B\nExec=b.exe\n",
        )
        .unwrap();
        fs::write(
            dir.join("a.desktop"),
            "[Desktop Entry]\nName=A\nExec=a.exe\n",
        )
        .unwrap();
        fs::write(dir.join("broken.desktop"), "[Desktop Entry]\nName=NoExec\n").unwrap();
        fs::write(dir.join("ignored.txt"), "Name=Nope\nExec=nope\n").unwrap();

        let entries = scan_desktop_files(&dir);
        assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            vec!["A", "B"]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_first_icon_from_desktop_entry_group() {
        let dir = temp_dir("icon-parse");
        let file = dir.join("Game.desktop");
        fs::write(
            &file,
            "[Desktop Entry]\nName=Game\nExec=game.exe\nIcon=0FEF_Vortex.0\nIcon=ignored\n[Desktop Action other]\nIcon=also-ignored\n",
        )
        .unwrap();

        let entry = parse_desktop_file(&file).unwrap();
        assert_eq!(entry.icon.as_deref(), Some("0FEF_Vortex.0"));
        assert_eq!(scan_desktop_files(&dir)[0].icon, entry.icon);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn orders_icon_sizes_by_closeness_to_32() {
        let dir = temp_dir("icon-order");
        for size in [16, 48, 64] {
            fs::create_dir_all(dir.join(format!("icons/{size}x{size}/apps"))).unwrap();
        }
        let candidates = icon_candidates(&dir, "Game");
        let names: Vec<String> = candidates
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            ["48x48", "64x64", "16x16"]
                .iter()
                .flat_map(|size| {
                    ["Game.png", "Game.ico", "Game"].map(|name| {
                        dir.join(format!("icons/{size}/apps/{name}"))
                            .to_string_lossy()
                            .into_owned()
                    })
                })
                .collect::<Vec<_>>()
        );

        let dir2 = temp_dir("icon-order-32");
        for size in [16, 32, 48] {
            fs::create_dir_all(dir2.join(format!("icons/{size}x{size}/apps"))).unwrap();
        }
        let first = icon_candidates(&dir2, "Game");
        assert!(first[0].to_string_lossy().contains("32x32"));
        assert!(first[3].to_string_lossy().contains("48x48"));
        assert!(first[6].to_string_lossy().contains("16x16"));
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn handles_direct_icon_paths_and_extensions() {
        let dir = temp_dir("icon-direct");
        assert_eq!(
            icon_candidates(&dir, r"C:\Games\icon"),
            vec![
                PathBuf::from(r"C:\Games\icon"),
                PathBuf::from(r"C:\Games\icon.png"),
                PathBuf::from(r"C:\Games\icon.ico"),
            ]
        );
        assert_eq!(
            icon_candidates(&dir, "/opt/game/icon.PNG"),
            vec![PathBuf::from("/opt/game/icon.PNG")]
        );

        for size in [16, 48] {
            fs::create_dir_all(dir.join(format!("icons/{size}x{size}/apps"))).unwrap();
        }
        assert_eq!(
            icon_candidates(&dir, "Game.png"),
            vec![
                dir.join("icons/48x48/apps/Game.png"),
                dir.join("icons/48x48/apps/Game"),
                dir.join("icons/16x16/apps/Game.png"),
                dir.join("icons/16x16/apps/Game"),
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_no_icon_candidates_without_icons_directory() {
        let dir = temp_dir("icon-missing");
        assert!(icon_candidates(&dir, "Game").is_empty());
        assert!(icon_candidates(&dir, "  ").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
