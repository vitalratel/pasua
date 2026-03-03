// ABOUTME: Configuration loading from file and environment variables.
// ABOUTME: Precedence: env var > config file (~/.config/pasua/config.toml) > built-in default.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

trait Env {
    fn var(&self, key: &str) -> Option<String>;
}

struct OsEnv;

impl Env for OsEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

/// Resolved configuration for a pasua run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Line delta threshold for auto-expanding a file's symbols.
    pub threshold: usize,
    /// Per-request LSP timeout.
    pub lsp_timeout: Duration,
    /// Timeout waiting for LSP initial indexing to complete.
    pub lsp_indexing_timeout: Duration,
    /// Per-language overrides for lsp_indexing_timeout, keyed by lsp_language_id().
    lsp_lang_indexing_timeouts: HashMap<String, Duration>,
}

#[derive(Deserialize, Default)]
struct TomlFile {
    defaults: Option<TomlDefaults>,
    lsp: Option<TomlLsp>,
}

#[derive(Deserialize)]
struct TomlDefaults {
    threshold: Option<usize>,
}

#[derive(Deserialize)]
struct TomlLsp {
    timeout: Option<u64>,
    indexing_timeout: Option<u64>,
    timeouts: Option<HashMap<String, u64>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            threshold: 200,
            lsp_timeout: Duration::from_secs(30),
            lsp_indexing_timeout: Duration::from_secs(30),
            lsp_lang_indexing_timeouts: HashMap::new(),
        }
    }
}

impl Config {
    /// Load config from file then apply env var overrides.
    pub fn load() -> Self {
        Self::resolve(config_path().as_ref(), &OsEnv)
    }

    fn resolve(path: Option<&PathBuf>, env: &impl Env) -> Self {
        let mut cfg = Self::default();
        if let Some(p) = path {
            cfg.apply_file(p);
        }
        cfg.apply_env(env);
        cfg
    }

    /// Indexing timeout for a specific language (falls back to global).
    pub fn lsp_indexing_timeout_for(&self, lang: &str) -> Duration {
        self.lsp_lang_indexing_timeouts
            .get(lang)
            .copied()
            .unwrap_or(self.lsp_indexing_timeout)
    }

    fn apply_file(&mut self, path: &PathBuf) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(file) = toml::from_str::<TomlFile>(&content) else {
            return;
        };
        if let Some(d) = file.defaults
            && let Some(t) = d.threshold
        {
            self.threshold = t;
        }
        if let Some(lsp) = file.lsp {
            if let Some(t) = lsp.timeout {
                self.lsp_timeout = Duration::from_secs(t);
            }
            if let Some(t) = lsp.indexing_timeout {
                self.lsp_indexing_timeout = Duration::from_secs(t);
            }
            if let Some(timeouts) = lsp.timeouts {
                for (lang, secs) in timeouts {
                    self.lsp_lang_indexing_timeouts
                        .insert(lang, Duration::from_secs(secs));
                }
            }
        }
    }

    fn apply_env(&mut self, env: &impl Env) {
        if let Some(t) = parse_var::<usize>(env, "PASUA_THRESHOLD") {
            self.threshold = t;
        }
        if let Some(t) = parse_var::<u64>(env, "PASUA_LSP_TIMEOUT") {
            self.lsp_timeout = Duration::from_secs(t);
        }
        if let Some(t) = parse_var::<u64>(env, "PASUA_LSP_INDEXING_TIMEOUT") {
            self.lsp_indexing_timeout = Duration::from_secs(t);
        }
        for lang in ["go", "rust", "python", "typescript", "elixir", "gleam"] {
            let key = format!("PASUA_LSP_{}_INDEXING_TIMEOUT", lang.to_uppercase());
            if let Some(t) = parse_var::<u64>(env, &key) {
                self.lsp_lang_indexing_timeouts
                    .insert(lang.to_string(), Duration::from_secs(t));
            }
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("pasua").join("config.toml"))
}

fn parse_var<T: std::str::FromStr>(env: &impl Env, key: &str) -> Option<T> {
    env.var(key)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    struct FakeEnv(HashMap<String, String>);

    impl Env for FakeEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
    }

    fn write_toml(content: &str) -> (NamedTempFile, PathBuf) {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn defaults_are_correct() {
        let cfg = Config::default();
        assert_eq!(cfg.threshold, 200);
        assert_eq!(cfg.lsp_timeout, Duration::from_secs(30));
        assert_eq!(cfg.lsp_indexing_timeout, Duration::from_secs(30));
    }

    #[test]
    fn toml_overrides_defaults() {
        let (_f, path) = write_toml(
            r#"
[defaults]
threshold = 500

[lsp]
timeout = 60
indexing_timeout = 120

[lsp.timeouts]
rust = 300
"#,
        );
        let cfg = Config::resolve(Some(&path), &FakeEnv(HashMap::new()));
        assert_eq!(cfg.threshold, 500);
        assert_eq!(cfg.lsp_timeout, Duration::from_secs(60));
        assert_eq!(cfg.lsp_indexing_timeout, Duration::from_secs(120));
        assert_eq!(
            cfg.lsp_indexing_timeout_for("rust"),
            Duration::from_secs(300)
        );
        assert_eq!(
            cfg.lsp_indexing_timeout_for("go"),
            Duration::from_secs(120),
            "go falls back to global"
        );
    }

    #[test]
    fn partial_toml_leaves_other_defaults_intact() {
        let (_f, path) = write_toml("[defaults]\nthreshold = 100\n");
        let cfg = Config::resolve(Some(&path), &FakeEnv(HashMap::new()));
        assert_eq!(cfg.threshold, 100);
        assert_eq!(cfg.lsp_timeout, Duration::from_secs(30), "unchanged");
    }

    #[test]
    fn env_overrides_toml() {
        let (_f, path) = write_toml("[defaults]\nthreshold = 500\n");
        let env = FakeEnv(HashMap::from([("PASUA_THRESHOLD".into(), "999".into())]));
        let cfg = Config::resolve(Some(&path), &env);
        assert_eq!(cfg.threshold, 999);
    }

    #[test]
    fn per_lang_env_override() {
        let env = FakeEnv(HashMap::from([(
            "PASUA_LSP_RUST_INDEXING_TIMEOUT".into(),
            "180".into(),
        )]));
        let cfg = Config::resolve(None, &env);
        assert_eq!(
            cfg.lsp_indexing_timeout_for("rust"),
            Duration::from_secs(180)
        );
        assert_eq!(
            cfg.lsp_indexing_timeout_for("go"),
            Duration::from_secs(30),
            "go unaffected"
        );
    }
}
