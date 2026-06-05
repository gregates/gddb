use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::canonicalize;
use std::path::PathBuf;
use std::sync::LazyLock;

use clap_complete::engine::CompletionCandidate;
use lib_gddb::arc::Archive;
use lib_gddb::tags;

use crate::{
    LANGUAGE,
    Language,
    TAG_DIR,
    TAG_FILE_PREFIX,
    TAG_EXT,
    DB_GD,
    DB_AOM,
    DB_FG,
    DB_FOA,
};
use crate::database::Database;

const XPAC_PATHS: [&'static str; 4] = [DB_GD, DB_AOM, DB_FG, DB_FOA];

pub const TAGS: LazyLock<HashMap<String, String>> = LazyLock::new(|| read_item_tags());

fn lang() -> Language {
    *LANGUAGE.get().unwrap()
}

pub fn xpac_db_path(xpac: usize) -> &'static str {
    if let Some(path) = XPAC_PATHS.get(xpac) {
        return path;
    }
    eprintln!("There is no gdx{xpac}");
    std::process::exit(1);
}

/// Returns the install path from GRIM_DAWN_INSTALL_PATH env var.
pub fn install_path() -> PathBuf {
    let relative_path = std::env::var_os("GRIM_DAWN_INSTALL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("Please set GRIM_DAWN_INSTALL_PATH");
            std::process::exit(1);
        });
    canonicalize(&relative_path).unwrap_or_else(|e| {
        eprintln!("Could not be resolve GRIM_DAWN_INSTALL_PATH={}: {}", relative_path.display(), e);
        std::process::exit(1);
    })
}

/// Returns the fully qualified path for a resource file specified relative to install path.
pub fn path_to(path: impl AsRef<str>) -> PathBuf {
    install_path().join(path.as_ref())
}

/// Returns the relative path to the text resource file for the specified xpac.
pub fn text_resource_path(xpac: usize) -> String {
    if xpac > 0 {
        format!("gdx{}/{}/{}{}{}", xpac, TAG_DIR, TAG_FILE_PREFIX, lang(), TAG_EXT)
    } else {
        format!("{}/{}{}{}", TAG_DIR, TAG_FILE_PREFIX, lang(), TAG_EXT)
    }
}

/// Returns the relative paths to text resource (`.arc`) files present under the
/// install path, across all expansions. With `language` set to a code such as
/// `"EN"`, only that language's files are returned; otherwise every language is
/// included. Used by grep, which searches text resources independently of the
/// selected language.
pub fn text_resource_paths(language: Option<Language>) -> Vec<String> {
    let wanted = language.map(|code| format!("{TAG_FILE_PREFIX}{code}{TAG_EXT}"));
    let mut paths = vec![];
    for xpac in 0..=3 {
        let dir = if xpac > 0 {
            format!("gdx{xpac}/{TAG_DIR}")
        } else {
            format!("{TAG_DIR}")
        };
        let Ok(entries) = std::fs::read_dir(path_to(&dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let matched = match &wanted {
                Some(wanted) => name.as_ref() == wanted.as_str(),
                None => name.starts_with(TAG_FILE_PREFIX) && name.ends_with(TAG_EXT),
            };
            if matched {
                paths.push(format!("{dir}/{name}"));
            }
        }
    }
    paths.sort();
    paths
}

fn read_item_tags() -> HashMap<String, String> {
    let item_tags = (0..=3)
        .into_iter()
        .map(|xpac| path_to(text_resource_path(xpac)))
        .enumerate()
        .filter_map(|(i, path)| Archive::open(&path).ok().map(|arc| (i, arc)))
        .map(|(i, mut arc)| {
            let filename = if i > 0 {
                format!("tagsgdx{i}_items.txt")
            } else {
                "tags_items.txt".to_string()
            };
            let item_tags = arc.get(filename.as_str()).unwrap();
            tags::parse(&item_tags.data).unwrap()
        })
        .reduce(|mut acc, tags| {
            acc.extend(tags.into_iter());
            acc
        });

    let Some(item_tags) = item_tags else {
        eprintln!(
            "Could not read tag files. Please verify GRIM_DAWN_INSTALL_PATH: {}",
            install_path().display()
        );
        std::process::exit(1);
    };

    item_tags
}

/// Given a set of record IDs and a prefix, returns the unique next path
/// components (directories suffixed with `/`, files without).
pub fn list_children(ids: impl Iterator<Item = String>, prefix: Option<&OsStr>) -> Vec<String> {
    let nexts: HashSet<String> = ids
        .filter_map(|id| {
            let path = PathBuf::from(&id);
            let path = match prefix {
                Some(prefix) => path.strip_prefix(prefix).ok()?.into(),
                None => path,
            };
            let mut path = path.into_iter();
            let next = path.next().map(|s| s.to_string_lossy().into_owned());
            match path.next() {
                Some(_) => next.map(|mut s| {
                    s.push('/');
                    s
                }),
                None => next,
            }
        })
        .collect();
    let mut sorted: Vec<String> = nexts.into_iter().collect();
    sorted.sort();
    sorted
}

/// Completer for record paths, used by shell tab completion.
pub fn complete_record_path(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(_) = std::env::var_os("GRIM_DAWN_INSTALL_PATH") else {
        return vec![];
    };

    let mut dbs = Database::load_all();

    let ids = dbs.iter_record_ids();
    let current_str = current.to_string_lossy();

    // Find the prefix directory portion (everything up to and including the last `/`)
    let prefix = if current_str.contains('/') {
        let last_slash = current_str.rfind('/').unwrap();
        Some(OsString::from(&current_str[..=last_slash]))
    } else {
        None
    };

    let children = list_children(ids, prefix.as_deref());

    let prefix_str = prefix.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();

    children
        .into_iter()
        .filter(|child| {
            // Filter to those matching the typed portion after the last `/`
            let suffix = &current_str[prefix_str.len()..];
            child.starts_with(suffix)
        })
        .map(|child| {
            let full = format!("{}{}", prefix_str, child);
            CompletionCandidate::new(full)
        })
        .collect()
}

