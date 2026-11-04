//! Application model plus a small, schema-specific YAML parser and serializer.

use crate::desktop::{scan_desktop_files, DesktopEntry};
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
            let parsed = parse(&String::from_utf8_lossy(&bytes));
            // An empty config almost always means the first run found no
            // .desktop files. Re-scan instead of locking the user into the
            // empty result.
            if !parsed.apps.is_empty() {
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

/// Parses the supported YAML subset. Unknown lines are skipped, so a partially
/// hand-edited file still yields every app it can understand.
pub fn parse(text: &str) -> LauncherConfig {
    let mut apps: Vec<AppConfig> = Vec::new();
    let mut mode = Mode::Scalar;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "apps:" {
            continue;
        }

        if let Some(rest) = line.strip_prefix("- ") {
            let (key, value) = split_key_value(rest);
            if is_app_key(key) {
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
            "args" => mode = Mode::Args,
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
