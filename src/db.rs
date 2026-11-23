//! Port of `PublisherDatabaseWrapper`: the `~/.vsce` publisher/PAT store.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::logger::Logger;
use crate::messages as msg;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublisherEntry {
    pub name: String,
    pub pat: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PublishersDatabase {
    #[serde(default)]
    publishers: Vec<PublisherEntry>,
}

pub struct PublisherDb {
    path: PathBuf,
}

/// `%USERPROFILE%\.vsce` (or `$HOME/.vsce`).
pub fn default_path() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_default()
        .join(".vsce")
}

impl PublisherDb {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self, log: &mut Logger) -> Option<PublishersDatabase> {
        let result = std::fs::read_to_string(&self.path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()));
        match result {
            Ok(db) => Some(db),
            Err(e) => {
                log.error(msg::ERROR_READING_PUBLISHER_DATABASE, &[&e]);
                None
            }
        }
    }

    fn write(&self, db: &PublishersDatabase, log: &mut Logger) -> bool {
        let text = serde_json::to_string(db).expect("serializable");
        match std::fs::write(&self.path, text) {
            Ok(()) => true,
            Err(e) => {
                log.error(msg::ERROR_WRITING_PUBLISHER_DATABASE, &[&e]);
                false
            }
        }
    }

    pub fn add(&self, name: &str, pat: &str, overwrite: bool, log: &mut Logger) -> bool {
        assert!(!name.is_empty(), "AddPublisherToDatabase requires a publisher name.");
        assert!(!pat.is_empty(), "AddPublisherToDatabase requires a personal access token.");
        let entry = PublisherEntry { name: name.into(), pat: pat.into() };
        let db = if !self.path.is_file() {
            PublishersDatabase { publishers: vec![entry] }
        } else {
            let Some(mut db) = self.read(log) else { return false };
            if let Some(i) = db.publishers.iter().position(|p| p.name == name) {
                if !overwrite {
                    log.error(msg::PUBLISHER_ALREADY_EXISTS, &[&name]);
                    return false;
                }
                db.publishers.remove(i);
            }
            db.publishers.push(entry);
            db
        };
        self.write(&db, log)
    }

    pub fn remove(&self, name: &str, ignore_missing: bool, log: &mut Logger) -> bool {
        let not_logged_in = |log: &mut Logger| {
            if ignore_missing {
                true
            } else {
                log.error(msg::PUBLISHER_NOT_LOGGED_IN, &[&name]);
                false
            }
        };
        if !self.path.is_file() {
            return not_logged_in(log);
        }
        let Some(mut db) = self.read(log) else { return false };
        let Some(i) = db.publishers.iter().position(|p| p.name == name) else {
            return not_logged_in(log);
        };
        db.publishers.remove(i);
        if db.publishers.is_empty() {
            if let Err(e) = std::fs::remove_file(&self.path) {
                log.error(msg::COULD_NOT_DELETE_PUBLISHER_DATABASE, &[&e]);
                return false;
            }
            true
        } else {
            self.write(&db, log)
        }
    }

    pub fn personal_access_token(&self, name: &str, log: &mut Logger) -> Option<String> {
        if !self.path.is_file() {
            log.error(msg::PUBLISHER_NOT_IN_VSCE, &[&name]);
            return None;
        }
        let db = self.read(log)?;
        match db.publishers.into_iter().find(|p| p.name == name).map(|p| p.pat) {
            Some(pat) if !pat.is_empty() => Some(pat),
            _ => {
                log.error(msg::PUBLISHER_NOT_IN_VSCE, &[&name]);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::test_support::capture;

    fn db() -> (tempfile::TempDir, PublisherDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = PublisherDb::new(dir.path().join(".vsce"));
        (dir, db)
    }

    #[test]
    fn add_lookup_remove() {
        let (_d, db) = db();
        let (mut log, _, _) = capture();
        assert!(db.add("a", "pat-a", false, &mut log));
        assert!(db.add("b", "pat-b", false, &mut log));
        assert_eq!(
            std::fs::read_to_string(db.path()).unwrap(),
            r#"{"publishers":[{"name":"a","pat":"pat-a"},{"name":"b","pat":"pat-b"}]}"#
        );
        assert_eq!(db.personal_access_token("b", &mut log).as_deref(), Some("pat-b"));
        assert!(db.remove("a", false, &mut log));
        assert!(db.remove("b", false, &mut log));
        assert!(!db.path().exists());
        assert!(log.last_error().is_none());
    }

    #[test]
    fn add_existing_requires_overwrite() {
        let (_d, db) = db();
        let (mut log, _, err) = capture();
        db.add("a", "1", false, &mut log);
        assert!(!db.add("a", "2", false, &mut log));
        assert_eq!(log.last_error_code_or(0), 12);
        assert!(err.text().contains("already logged-in: 'a'"));
        assert!(db.add("a", "2", true, &mut log));
        assert_eq!(db.personal_access_token("a", &mut log).as_deref(), Some("2"));
    }

    #[test]
    fn remove_missing() {
        let (_d, db) = db();
        let (mut log, _, _) = capture();
        assert!(db.remove("x", true, &mut log));
        assert!(log.last_error().is_none());
        assert!(!db.remove("x", false, &mut log));
        assert_eq!(log.last_error_code_or(0), 14);
        db.add("a", "1", false, &mut log);
        assert!(!db.remove("x", false, &mut log));
        assert!(db.path().exists());
    }

    #[test]
    fn lookup_missing_publisher() {
        let (_d, db) = db();
        let (mut log, _, _) = capture();
        assert!(db.personal_access_token("x", &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 7);
        db.add("a", "1", false, &mut log);
        assert!(db.personal_access_token("x", &mut log).is_none());
    }

    #[test]
    fn corrupt_file_is_reported() {
        let (_d, db) = db();
        std::fs::write(db.path(), "nope").unwrap();
        let (mut log, _, _) = capture();
        assert!(db.personal_access_token("x", &mut log).is_none());
        assert_eq!(log.last_error_code_or(0), 8);
    }
}
