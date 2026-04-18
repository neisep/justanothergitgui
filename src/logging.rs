use std::collections::VecDeque;
use std::env;
use std::fs::{self, OpenOptions};
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
    pub fn new() -> Self {
        let path = default_log_path();
        let logger = Self {
            path,
            fallback_entries: Mutex::new(VecDeque::new()),
        };
        if let Some(parent) = logger.path.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!(
                "Logger failure: could not create log directory {}: {}",
                parent.display(),
                error
            );
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
    use super::AppLogger;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

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
