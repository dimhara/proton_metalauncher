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
            "Exec" if exec.is_none() => exec = Some(unescape(value)),
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
        path: cwd.filter(|value| !value.is_empty()),
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

/// Splits an `Exec=` value into program and arguments, respecting single and
/// double quotes and dropping `%`-field codes.
fn split_exec(exec: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;

    for c in exec.chars() {
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
        .collect()
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
