//! Application model plus a small, schema-specific YAML parser and serializer.

use crate::desktop::{scan_desktop_files, split_windows_args, DesktopEntry};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AppConfig {
    pub name: String,
    pub path: String,
    pub cwd: Option<String>,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl AppConfig {
    fn from_desktop(entry: DesktopEntry) -> Self {
        AppConfig {
            name: entry.name,
            path: entry.exec,
            cwd: entry.path,
            args: entry.args,
            env: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LauncherConfig {
    pub apps: Vec<AppConfig>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Scalar,
    Args,
    Env,
}

impl LauncherConfig {
    /// Loads `<dir>/config.yaml`, generating it from `*.desktop` files first if
    /// it does not exist yet.
    pub fn load_or_create(dir: &Path) -> Result<LauncherConfig, String> {
        let file = dir.join("config.yaml");
        if file.exists() {
            let bytes =
                fs::read(&file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
            let text = decode_text(&bytes);
            let parsed = parse(&text);
            // An empty config almost always means the first run found no
            // .desktop files. Re-scan instead of locking the user into the
            // empty result, but never overwrite a hand-edited file we could
            // not understand.
            if !parsed.apps.is_empty() || !is_effectively_empty(&text) {
                return Ok(parsed);
            }
        }

        let config = LauncherConfig {
            apps: scan_desktop_files(dir)
                .into_iter()
                .map(AppConfig::from_desktop)
                .collect(),
        };

        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        fs::write(&file, config.to_yaml())
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
        Ok(config)
    }

    /// Re-reads `config.yaml` without generating or modifying anything.
    /// Returns `None` when the file is missing or unreadable.
    pub fn load_existing(dir: &Path) -> Option<LauncherConfig> {
        let bytes = fs::read(dir.join("config.yaml")).ok()?;
        Some(parse(&decode_text(&bytes)))
    }

    /// Serializes the configuration using the canonical layout.
    pub fn to_yaml(&self) -> String {
        let mut out = String::from("apps:\n");
        for app in &self.apps {
            out.push_str("  - name: ");
            out.push_str(&quote(&app.name));
            out.push('\n');
            out.push_str("    path: ");
            out.push_str(&quote(&app.path));
            out.push('\n');
            out.push_str("    cwd: ");
            out.push_str(&quote(app.cwd.as_deref().unwrap_or("")));
            out.push('\n');
            out.push_str("    args:\n");
            for arg in &app.args {
                out.push_str("      - ");
                out.push_str(&quote(arg));
                out.push('\n');
            }
            out.push_str("    env:\n");
            for (key, value) in &app.env {
                out.push_str("      ");
                out.push_str(key);
                out.push_str(": ");
                out.push_str(&quote(value));
                out.push('\n');
            }
        }
        out
    }
}

fn is_app_key(key: &str) -> bool {
    matches!(key, "name" | "path" | "cwd" | "args" | "env")
}

/// Decodes UTF-8 (with or without BOM) and the UTF-16 forms Notepad can write.
fn decode_text(bytes: &[u8]) -> String {
    let text = if bytes.starts_with(&[0xFF, 0xFE]) {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
}

/// True when a config only contains the `apps:` header, comments and blank
/// lines. Such a file is safe to regenerate from `.desktop` files.
fn is_effectively_empty(text: &str) -> bool {
    text.lines().all(|raw| {
        let line = strip_comment(raw.trim()).trim();
        line.is_empty() || line == "apps:" || line == "apps: []"
    })
}

/// Strips a trailing YAML comment. A `#` starts a comment only at the start of
/// the line or after whitespace, and never inside quotes.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if q == '"' && c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => {
                let at_start = index == 0
                    || line[..index]
                        .chars()
                        .next_back()
                        .is_none_or(char::is_whitespace);
                if at_start {
                    return line[..index].trim_end();
                }
            }
            None => {}
        }
    }
    line
}

/// Splits the inside of an inline YAML list (`["a", "b"]`) on top-level
/// commas, ignoring commas inside quotes.
fn split_flow_items(value: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in value.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if q == '"' && c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ',' => {
                items.push(&value[start..index]);
                start = index + c.len_utf8();
            }
            None => {}
        }
    }
    items.push(&value[start..]);
    items
}

/// Arguments written on the `args:` line itself. Both an inline list and a
/// single scalar command line are accepted; the canonical form is a block
/// list on the following lines.
fn parse_inline_args(value: &str) -> Vec<String> {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        split_flow_items(inner)
            .into_iter()
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(unquote)
            .collect()
    } else {
        split_windows_args(&unquote(value))
    }
}

/// Parses the supported YAML subset. Unknown lines are skipped, so a partially
/// hand-edited file still yields every app it can understand.
pub fn parse(text: &str) -> LauncherConfig {
    let mut apps: Vec<AppConfig> = Vec::new();
    let mut mode = Mode::Scalar;

    for raw in text.lines() {
        let line = strip_comment(raw.trim()).trim();
        if line.is_empty() {
            continue;
        }
        if line == "apps:" {
            continue;
        }

        if let Some(rest) = line.strip_prefix("- ") {
            let (key, value) = split_key_value(rest);
            // A `- ` line is an app only when it is `- key: ...` with a known
            // key; otherwise it is an argument (e.g. `- "-dx11"`).
            if is_app_key(key) && rest.contains(':') {
                apps.push(AppConfig::default());
                mode = Mode::Scalar;
                set_scalar(apps.last_mut(), key, &value);
            } else if mode == Mode::Args {
                if let Some(app) = apps.last_mut() {
                    app.args.push(unquote(rest));
                }
            }
            continue;
        }

        let (key, value) = split_key_value(line);
        match key {
            "name" | "path" | "cwd" => {
                set_scalar(apps.last_mut(), key, &value);
                mode = Mode::Scalar;
            }
            "args" => {
                if let Some(app) = apps.last_mut() {
                    app.args.extend(parse_inline_args(&value));
                }
                mode = Mode::Args;
            }
            "env" => mode = Mode::Env,
            _ => {
                if mode == Mode::Env {
                    if let Some(app) = apps.last_mut() {
                        app.env.push((key.to_string(), unquote(&value)));
                    }
                }
            }
        }
    }

    LauncherConfig { apps }
}

fn set_scalar(app: Option<&mut AppConfig>, key: &str, value: &str) {
    let Some(app) = app else { return };
    match key {
        "name" => app.name = unquote(value),
        "path" => app.path = unquote(value),
        "cwd" => {
            let cwd = unquote(value);
            app.cwd = if cwd.is_empty() { None } else { Some(cwd) };
        }
        _ => {}
    }
}

/// Splits `key: value` at the first colon. Lines without a colon get an empty
/// value.
fn split_key_value(s: &str) -> (&str, String) {
    match s.find(':') {
        Some(i) => (s[..i].trim(), s[i + 1..].trim().to_string()),
        None => (s.trim(), String::new()),
    }
}

/// Removes one layer of YAML quoting and processes double-quoted escapes.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        let mut out = String::with_capacity(s.len() - 2);
        let mut chars = s[1..s.len() - 1].chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        }
        out
    } else if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "apps:\n  - name: \"Game One\"\n    path: \"C:\\\\Games\\\\Game1\\\\game.exe\"\n    cwd: \"C:\\\\Games\\\\Game1\"\n    args:\n      - \"-dx11\"\n    env:\n      DXVK_HUD: \"fps\"\n";

    #[test]
    fn parses_reference_sample() {
        let config = parse(SAMPLE);
        assert_eq!(config.apps.len(), 1);
        let app = &config.apps[0];
        assert_eq!(app.name, "Game One");
        assert_eq!(app.path, r"C:\Games\Game1\game.exe");
        assert_eq!(app.cwd.as_deref(), Some(r"C:\Games\Game1"));
        assert_eq!(app.args, vec!["-dx11"]);
        assert_eq!(app.env, vec![("DXVK_HUD".to_string(), "fps".to_string())]);
    }

    #[test]
    fn round_trips() {
        let config = parse(SAMPLE);
        assert_eq!(parse(&config.to_yaml()), config);
        assert_eq!(config.to_yaml(), SAMPLE);
    }

    #[test]
    fn tolerates_comments_blank_lines_and_unquoted_values() {
        let text = "# comment\napps:\n- name: Quoted App\n  path: C:\\Games\\a.exe\n  cwd:\n  args:\n  - --fullscreen\n  - -novid\n  env:\n    WINEDLLOVERRIDES: d3d11=n,b\n\n";
        let config = parse(text);
        assert_eq!(config.apps.len(), 1);
        let app = &config.apps[0];
        assert_eq!(app.name, "Quoted App");
        assert_eq!(app.cwd, None);
        assert_eq!(app.args, vec!["--fullscreen", "-novid"]);
        assert_eq!(
            app.env,
            vec![("WINEDLLOVERRIDES".to_string(), "d3d11=n,b".to_string())]
        );
    }

    #[test]
    fn parses_multiple_apps() {
        let text = "apps:\n  - name: \"A\"\n    path: \"a.exe\"\n    cwd: \"\"\n    args:\n    env:\n  - name: \"B\"\n    path: \"b.exe\"\n    cwd: \"\"\n    args:\n      - \"--x\"\n    env:\n      FOO: \"bar\"\n";
        let config = parse(text);
        assert_eq!(config.apps.len(), 2);
        assert_eq!(config.apps[0].name, "A");
        assert_eq!(config.apps[0].cwd, None);
        assert_eq!(config.apps[1].name, "B");
        assert_eq!(config.apps[1].args, vec!["--x"]);
        assert_eq!(config.apps[1].env.len(), 1);
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("metalauncher-config-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn generates_config_from_desktop_files_once() {
        let dir = temp_dir("generate");
        fs::write(
            dir.join("Game.desktop"),
            "[Desktop Entry]\nName=Game\nExec=\"C:\\Games\\g.exe\" -dx11\nPath=C:\\Games\n",
        )
        .unwrap();

        let config = LauncherConfig::load_or_create(&dir).unwrap();
        assert_eq!(config.apps.len(), 1);
        assert_eq!(config.apps[0].name, "Game");
        assert_eq!(config.apps[0].args, vec!["-dx11"]);

        let written = fs::read_to_string(dir.join("config.yaml")).unwrap();
        assert!(
            written.starts_with("apps:\n  - name: \"Game\"\n"),
            "{written}"
        );
        assert!(written.contains(r#"path: "C:\\Games\\g.exe""#), "{written}");
        assert!(written.contains(r#"cwd: "C:\\Games""#), "{written}");
        assert!(written.contains("    env:\n"), "{written}");

        // Once config.yaml exists, the .desktop files are no longer consulted.
        fs::remove_file(dir.join("Game.desktop")).unwrap();
        assert_eq!(LauncherConfig::load_or_create(&dir).unwrap(), config);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generated_config_keeps_shortcut_argument_boundaries() {
        let dir = temp_dir("shortcut-args");
        fs::write(
            dir.join("Game.desktop"),
            r#"[Desktop Entry]
Name=Game
Exec="C:\\\\Games\\\\g.exe" "-mode 1 -log \\"my log.txt\\""
"#,
        )
        .unwrap();

        let config = LauncherConfig::load_or_create(&dir).unwrap();
        assert_eq!(
            config.apps[0].args,
            vec!["-mode", "1", "-log", "my log.txt"]
        );

        let written = fs::read_to_string(dir.join("config.yaml")).unwrap();
        assert!(written.contains("      - \"my log.txt\"\n"), "{written}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_inline_argument_list() {
        let config =
            parse("apps:\n  - name: \"G\"\n    path: \"g.exe\"\n    args: [\"-a\", \"-b,c\"]\n");
        assert_eq!(config.apps[0].args, vec!["-a", "-b,c"]);
    }

    #[test]
    fn parses_scalar_arguments_as_a_command_line() {
        let config = parse("apps:\n  - name: \"G\"\n    path: \"g.exe\"\n    args: \"-a -b c\"\n");
        assert_eq!(config.apps[0].args, vec!["-a", "-b", "c"]);

        let config = parse("apps:\n  - name: \"G\"\n    path: \"g.exe\"\n    args: -a -b\n");
        assert_eq!(config.apps[0].args, vec!["-a", "-b"]);
    }

    #[test]
    fn strips_comments_and_keeps_quoted_hashes() {
        let config = parse(
            "apps:\n  - name: \"G\" # app\n    path: \"g.exe\"\n    args:\n      - \"-a#1\" # note\n      - \"-b #2\"\n",
        );
        assert_eq!(config.apps[0].name, "G");
        assert_eq!(config.apps[0].args, vec!["-a#1", "-b #2"]);
    }

    #[test]
    fn argument_named_like_a_key_stays_an_argument() {
        let config =
            parse("apps:\n  - name: \"G\"\n    path: \"g.exe\"\n    args:\n      - name\n");
        assert_eq!(config.apps.len(), 1);
        assert_eq!(config.apps[0].args, vec!["name"]);
    }

    #[test]
    fn reads_utf16_config_files() {
        let dir = temp_dir("utf16");
        let text = "apps:\n  - name: \"G\"\n    path: \"g.exe\"\n    args:\n      - \"-u16\"\n";
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(dir.join("config.yaml"), &bytes).unwrap();

        let config = LauncherConfig::load_or_create(&dir).unwrap();
        assert_eq!(config.apps[0].args, vec!["-u16"]);
        assert_eq!(fs::read(dir.join("config.yaml")).unwrap(), bytes);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_existing_sees_edited_arguments() {
        let dir = temp_dir("reload");
        let file = dir.join("config.yaml");
        let write = |arg: &str| {
            fs::write(
                &file,
                format!(
                    "apps:\n  - name: \"G\"\n    path: \"g.exe\"\n    args:\n      - \"{arg}\"\n"
                ),
            )
            .unwrap();
        };

        write("-old");
        assert_eq!(
            LauncherConfig::load_existing(&dir).unwrap().apps[0].args,
            vec!["-old"]
        );
        write("-new");
        assert_eq!(
            LauncherConfig::load_existing(&dir).unwrap().apps[0].args,
            vec!["-new"]
        );
        assert!(LauncherConfig::load_existing(&dir.join("missing")).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unparsable_config_is_not_overwritten() {
        let dir = temp_dir("keep");
        let text = "apps:\n  name: \"G\"\n  path: \"g.exe\"\n  args:\n    - \"-x\"\n";
        fs::write(dir.join("config.yaml"), text).unwrap();
        fs::write(
            dir.join("Found.desktop"),
            "[Desktop Entry]\nName=Found\nExec=found.exe\n",
        )
        .unwrap();

        let config = LauncherConfig::load_or_create(&dir).unwrap();
        assert!(config.apps.is_empty());
        assert_eq!(fs::read_to_string(dir.join("config.yaml")).unwrap(), text);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_config_is_regenerated_when_desktop_files_appear() {
        let dir = temp_dir("empty");
        let config = LauncherConfig::load_or_create(&dir).unwrap();
        assert!(config.apps.is_empty());
        assert_eq!(
            fs::read_to_string(dir.join("config.yaml")).unwrap(),
            "apps:\n"
        );

        fs::write(
            dir.join("Later.desktop"),
            "[Desktop Entry]\nName=Later\nExec=/tmp/later\n",
        )
        .unwrap();
        let config = LauncherConfig::load_or_create(&dir).unwrap();
        assert_eq!(config.apps.len(), 1);
        assert_eq!(config.apps[0].name, "Later");
        assert!(fs::read_to_string(dir.join("config.yaml"))
            .unwrap()
            .contains("Later"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn escapes_are_symmetric() {
        let app = AppConfig {
            name: "quo\"te".to_string(),
            path: r"C:\a b\x.exe".to_string(),
            cwd: None,
            args: vec!["a\\b".to_string(), "line\nbreak".to_string()],
            env: vec![("K".to_string(), "v\"1".to_string())],
        };
        let config = LauncherConfig { apps: vec![app] };
        assert_eq!(parse(&config.to_yaml()), config);
    }
}
