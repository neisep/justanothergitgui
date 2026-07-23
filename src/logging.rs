use std::collections::VecDeque;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const NO_LOGS_MESSAGE: &str = "No logs written yet.";
const FALLBACK_LOG_HEADER: &str = "Buffered log entries:\n";
const FALLBACK_BUFFER_LIMIT: usize = 100;

pub struct AppLogger {
    path: PathBuf,
    fallback_entries: Mutex<VecDeque<String>>,
}

impl AppLogger {
    /// App-level logger for events not tied to a specific repository
    /// (settings, GitHub sign-in, clone/publish, session).
    pub fn new() -> Self {
        Self::with_path(default_log_path())
    }

    /// Per-repository logger so each open tab keeps its own log file and the
    /// "View Logs" button only surfaces logs for the repository in view.
    pub fn for_repo(repo_path: &Path) -> Self {
        Self::with_path(log_dir().join(repo_log_file_name(repo_path)))
    }

    fn with_path(path: PathBuf) -> Self {
        let logger = Self {
            path,
            fallback_entries: Mutex::new(VecDeque::new()),
        };
        if let Some(parent) = logger.path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!("Logger failure: could not create log directory: {}", error);
        }
        logger
    }

    pub fn has_entries(&self) -> bool {
        fs::metadata(&self.path)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false)
            || self.has_fallback_entries()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_entries(&self) -> String {
        let fallback = self.fallback_entries_text();

        if !self.path.exists() {
            return fallback
                .map(|entries| format!("{FALLBACK_LOG_HEADER}{entries}"))
                .unwrap_or_else(|| NO_LOGS_MESSAGE.into());
        }

        match fs::read_to_string(&self.path) {
            Ok(contents) if contents.trim().is_empty() => fallback
                .map(|entries| format!("{FALLBACK_LOG_HEADER}{entries}"))
                .unwrap_or_else(|| NO_LOGS_MESSAGE.into()),
            Ok(contents) => match fallback {
                Some(entries) => format!("{contents}\n\n{FALLBACK_LOG_HEADER}{entries}"),
                None => contents,
            },
            Err(error) => match fallback {
                Some(entries) => format!(
                    "Could not read log file: {}\n\n{FALLBACK_LOG_HEADER}{entries}",
                    error
                ),
                None => format!("Could not read log file: {}", error),
            },
        }
    }

    pub fn clear_entries(&self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_file(&self.path)
                .map_err(|error| format!("Could not clear log file: {}", error))?;
        }

        self.clear_fallback_entries();
        Ok(())
    }

    pub fn log_error(&self, context: &str, detail: &str) {
        let sanitized = sanitize_log_text(detail);
        let line = format!("[{}] {}: {}\n", unix_timestamp(), context, sanitized);
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(line.as_bytes()) {
                    self.write_to_fallback(&line, &format!("Could not write log file: {}", error));
                }
            }
            Err(error) => {
                self.write_to_fallback(&line, &format!("Could not open log file: {}", error));
            }
        }
    }

    fn has_fallback_entries(&self) -> bool {
        let entries = self
            .fallback_entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        !entries.is_empty()
    }

    fn fallback_entries_text(&self) -> Option<String> {
        let entries = self
            .fallback_entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if entries.is_empty() {
            None
        } else {
            Some(entries.iter().cloned().collect::<Vec<_>>().join(""))
        }
    }

    fn clear_fallback_entries(&self) {
        self.fallback_entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }

    fn write_to_fallback(&self, line: &str, failure: &str) {
        eprintln!("Logger failure: {failure}\n{line}");

        let mut entries = self
            .fallback_entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.push_back(format!(
            "[{}] Logger failure: {}\n{}",
            unix_timestamp(),
            failure,
            line
        ));

        while entries.len() > FALLBACK_BUFFER_LIMIT {
            entries.pop_front();
        }
    }
}

pub fn summarize_for_ui(detail: &str) -> String {
    let sanitized = sanitize_log_text(detail);
    let first_line = sanitized
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("Unknown error");

    truncate(first_line, 96)
}

pub fn sanitize_log_text(detail: &str) -> String {
    let mut sanitized = detail.replace('\r', "");
    sanitized = redact_url_userinfo(&sanitized);

    for prefix in ["Bearer ", "bearer "] {
        sanitized = redact_after_prefix(&sanitized, prefix, &[' ', '\n', '\t', '"', '\'']);
    }

    for prefix in [
        "access_token=",
        "token=",
        "password=",
        "passwd=",
        "client_secret=",
        "\"access_token\":\"",
        "\"token\":\"",
        "\"password\":\"",
        "'access_token':'",
        "'token':'",
        "'password':'",
    ] {
        sanitized = redact_after_prefix(
            &sanitized,
            prefix,
            &['&', ' ', '\n', '\t', '"', '\'', ',', '}'],
        );
    }

    sanitized
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut end = 0;
    let mut chars = 0;
    for (idx, ch) in text.char_indices() {
        if chars == max_chars {
            end = idx;
            break;
        }
        chars += 1;
        end = idx + ch.len_utf8();
    }

    if chars <= max_chars {
        text.to_string()
    } else {
        format!("{}...", &text[..end])
    }
}

fn redact_after_prefix(text: &str, prefix: &str, terminators: &[char]) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find(prefix) {
        let prefix_end = start + prefix.len();
        output.push_str(&rest[..prefix_end]);
        output.push_str("[REDACTED]");

        let suffix = &rest[prefix_end..];
        let end = suffix
            .find(|ch| terminators.contains(&ch))
            .unwrap_or(suffix.len());
        rest = &suffix[end..];
    }

    output.push_str(rest);
    output
}

fn redact_url_userinfo(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(scheme_idx) = rest.find("://") {
        let authority_start = scheme_idx + 3;
        output.push_str(&rest[..authority_start]);

        let suffix = &rest[authority_start..];
        let authority_end = suffix
            .find(|ch: char| ['/', ' ', '\n', '\t', '"', '\''].contains(&ch))
            .unwrap_or(suffix.len());
        let authority = &suffix[..authority_end];

        if let Some(at_idx) = authority.rfind('@') {
            output.push_str("[REDACTED]@");
            output.push_str(&authority[at_idx + 1..]);
        } else {
            output.push_str(authority);
        }

        rest = &suffix[authority_end..];
    }

    output.push_str(rest);
    output
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn default_log_path() -> PathBuf {
    log_dir().join("app.log")
}

/// Build a stable, filesystem-safe log file name for a repository. The path
/// hash keeps distinct repositories separate even when they share a folder
/// name, while the readable slug makes the file easy to identify on disk.
fn repo_log_file_name(repo_path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    repo_path.hash(&mut hasher);
    let hash = hasher.finish();

    let slug = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(sanitize_file_component)
        .filter(|slug| !slug.is_empty())
        .unwrap_or_else(|| "repo".to_string());

    format!("repo-{slug}-{hash:016x}.log")
}

fn sanitize_file_component(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn log_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = env::var_os("APPDATA") {
            return PathBuf::from(appdata)
                .join("justanothergitgui")
                .join("logs");
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Logs")
                .join("justanothergitgui");
        }
    }

    if let Some(state_home) = env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home).join("justanothergitgui");
    }

    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("justanothergitgui");
    }

    env::temp_dir().join("justanothergitgui")
}

#[cfg(test)]
mod tests {
    use super::{AppLogger, repo_log_file_name};
    use std::collections::VecDeque;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn repo_log_file_name_differs_per_repository() {
        let a = repo_log_file_name(Path::new("/home/me/project-a"));
        let b = repo_log_file_name(Path::new("/home/me/project-b"));

        assert_ne!(a, b);
        assert!(a.starts_with("repo-project-a-"));
        assert!(a.ends_with(".log"));
    }

    #[test]
    fn repo_log_file_name_is_stable_for_same_repository() {
        let path = Path::new("/home/me/project");
        assert_eq!(repo_log_file_name(path), repo_log_file_name(path));
    }

    #[test]
    fn repo_log_file_name_distinguishes_same_folder_name_in_different_paths() {
        let a = repo_log_file_name(Path::new("/home/me/work/app"));
        let b = repo_log_file_name(Path::new("/home/me/personal/app"));

        // Same readable slug, but the path hash keeps the files separate.
        assert_ne!(a, b);
        assert!(a.starts_with("repo-app-"));
        assert!(b.starts_with("repo-app-"));
    }

    #[test]
    fn repo_log_file_name_sanitizes_unsafe_characters() {
        let name = repo_log_file_name(Path::new("/tmp/weird name!/repo@#"));

        assert!(name.starts_with("repo-repo__-"));
        let slug_and_hash = name.trim_start_matches("repo-").trim_end_matches(".log");
        assert!(
            slug_and_hash
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        );
    }

    #[test]
    fn clear_entries_removes_log_file() {
        let path = unique_test_path("clear");
        let logger = AppLogger {
            path: path.clone(),
            fallback_entries: Mutex::new(VecDeque::new()),
        };

        fs::write(&path, "test log entry\n").unwrap();
        assert!(logger.has_entries());

        logger.clear_entries().unwrap();

        assert!(!path.exists());
        assert!(!logger.has_entries());
        assert_eq!(logger.read_entries(), "No logs written yet.");
    }

    #[test]
    fn read_entries_treats_empty_file_as_no_logs() {
        let path = unique_test_path("empty");
        let logger = AppLogger {
            path: path.clone(),
            fallback_entries: Mutex::new(VecDeque::new()),
        };

        fs::write(&path, "").unwrap();

        assert_eq!(logger.read_entries(), "No logs written yet.");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn log_error_falls_back_to_memory_when_log_file_is_unavailable() {
        let path = unique_missing_parent_path("fallback");
        let logger = AppLogger {
            path,
            fallback_entries: Mutex::new(VecDeque::new()),
        };

        logger.log_error("Settings", "disk full");

        let entries = logger.read_entries();
        assert!(logger.has_entries());
        assert!(entries.contains("Logger failure: Could not open log file:"));
        assert!(entries.contains("Settings: disk full"));

        logger.clear_entries().unwrap();
        assert!(!logger.has_entries());
        assert_eq!(logger.read_entries(), "No logs written yet.");
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("justanothergitgui-{name}-{nanos}.log"))
    }

    fn unique_missing_parent_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("justanothergitgui-{name}-{nanos}"))
            .join("app.log")
    }
}
