//! Minimal freedesktop `.desktop` parser: extracts `Name=`, `Exec=` and `Path=`
//! from the `[Desktop Entry]` group.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub path: Option<String>,
    pub args: Vec<String>,
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
    })
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
/// The files Proton tooling generates are not strictly spec-compliant: Windows
/// paths are double-escaped (`C:\\\\Games`), and spaces are escaped as `\ ` rather
/// than `\s`. Both forms are handled here.
fn split_exec(exec: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = exec.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {
                current.push(c);
                started = true;
            }
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c == '\\' => {
                let mut count = 1;
                while chars.peek() == Some(&'\\') {
                    chars.next();
                    count += 1;
                }
                match chars.peek() {
                    Some(next) if next.is_whitespace() => {
                        // `\ ` (or `\\ `) is an escaped space.
                        current.push(' ');
                        chars.next();
                        started = true;
                    }
                    Some('"') | Some('\'') => {
                        current.push(chars.next().unwrap());
                        started = true;
                    }
                    _ => {
                        if current.is_empty() && count >= 2 {
                            // Keep a UNC prefix intact.
                            current.push_str("\\\\");
                        } else {
                            current.push('\\');
                        }
                        started = true;
                    }
                }
            }
            None if c.is_whitespace() => {
                if started {
                    parts.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        parts.push(current);
    }

    parts
        .into_iter()
        .filter(|part| !part.starts_with('%'))
        .map(|part| decode_arg(&part))
        .collect()
}

fn decode_arg(part: &str) -> String {
    if is_windows_path(part) {
        normalize_windows_path(part)
    } else {
        unescape(part)
    }
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
}
