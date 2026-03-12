use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek};
use std::path::PathBuf;
use std::sync::LazyLock;

use clap_complete::engine::CompletionCandidate;
use lib_gddb::arc::Archive;
use lib_gddb::arz::{Database, DatabaseValue, RawRecord, Record};
use lib_gddb::tags;

use crate::{
    LANGUAGE,
    Language,
    TAG_FILE,
    TAG_EXT,
    DB_GD,
    DB_AOM,
    DB_FG,
    DB_FOA,
};

pub const TAGS: LazyLock<HashMap<String, String>> = LazyLock::new(|| read_item_tags());

/// Yields all records that satisfy predicate p from the provided
/// databases.
pub fn iter_records<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    p: impl Fn(&str, &RawRecord) -> bool,
) -> impl Iterator<Item = Record> + '_ {
    records_by_xpac(arz, p)
        .into_iter()
        .map(|db| db.into_iter())
        .flatten()
}

/// Yields all record ids from the provided databases.
pub fn iter_record_ids<T: BufRead + Seek>(
    arz: &mut [Database<T>],
) -> impl Iterator<Item = String> + '_ {
    match load_raws_by_xpac(arz)
        .into_iter()
        .enumerate()
        .map(|(i, raws)| {
            raws.into_iter()
                .map(|raw| arz[i].record_id(&raw))
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(ids) => ids.into_iter().flat_map(|ids| ids.into_iter()),
        Err(e) => {
            eprintln!("Error parsing database records: {e}");
            std::process::exit(1);
        }
    }
}

/// Gets a specific record from any of the provided databases by id.
/// If more than one record with that id exists, the last one is
/// returned.
pub fn get_record<T: BufRead + Seek>(arz: &mut [Database<T>], matches: OsString) -> Record {
    let needle = matches.to_string_lossy();
    let mut matches = iter_records(arz, |id, _| id == needle).collect::<Vec<_>>();
    if matches.is_empty() {
        eprintln!("not found: {needle}");
        std::process::exit(1);
    } else if matches.len() > 1 {
        eprintln!(
            "WARN: {} records found for {}; showing latest",
            matches.len(),
            needle
        );
    }
    matches.pop().expect("record.len() > 0")
}



fn records_by_xpac<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    p: impl Fn(&str, &RawRecord) -> bool,
) -> Vec<Vec<Record>> {
    match load_raws_by_xpac(arz)
        .into_iter()
        .enumerate()
        .map(|(i, raws)| {
            raws.into_iter()
                .filter_map(|raw| {
                    let id = arz[i].record_id(&raw).ok()?;
                    if p(id.as_str(), &raw) {
                        Some(arz[i].resolve(raw))
                    } else {
                        None
                    }
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(records) => records,
        Err(e) => {
            eprintln!("Error parsing database records: {e}");
            std::process::exit(1);
        }
    }
}

fn load_raws_by_xpac<T: BufRead + Seek>(arz: &mut [Database<T>]) -> Vec<Vec<RawRecord>> {
    arz.iter_mut()
        .map(|db| {
            db.iter_records()
                .unwrap()
                .map(|result| result.unwrap())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

fn lang() -> Language {
    *LANGUAGE.get().unwrap()
}

/// Returns the install path from GRIM_DAWN_INSTALL_PATH env var.
pub fn install_path() -> PathBuf {
    std::env::var_os("GRIM_DAWN_INSTALL_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("Please set GRIM_DAWN_INSTALL_PATH");
            std::process::exit(1);
        })
}

/// Returns the fully qualified path for a resource file specified relative to install path.
pub fn path_to(path: impl AsRef<str>) -> PathBuf {
    install_path().join(path.as_ref())
}

/// Returns the relative path to the text resource file for the specified xpac.
pub fn text_resource(xpac: usize) -> String {
    if xpac > 0 {
        format!("gdx{}/{}{}{}", xpac, TAG_FILE, lang(), TAG_EXT)
    } else {
        format!("{}{}{}", TAG_FILE, lang(), TAG_EXT)
    }
}

fn read_item_tags() -> HashMap<String, String> {
    let item_tags = (0..=3)
        .into_iter()
        .map(|xpac| path_to(text_resource(xpac)))
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

    let mut dbs: Vec<Database<BufReader<File>>> = [DB_GD, DB_AOM, DB_FG, DB_FOA]
        .iter()
        .filter_map(|p| Database::open(&path_to(p)).ok())
        .collect();

    let ids = iter_record_ids(&mut dbs);
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

/// Attempts to find the set of records for an item by item name.
/// If a partial item name is given that matches more than one tag, the matches are printed and the program exits.
/// If the partial item name matches a unique tag, that tag is returned along with the set of records that reference it.
pub fn lookup_item<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    item: OsString,
) -> (String, HashMap<String, Record>) {
    let item = item.to_string_lossy();
    let item_parts = item.split_ascii_whitespace().collect::<Vec<_>>();
    let tags = &*TAGS;
    let mut possible_tags = vec![];
    for (tag, value) in tags.iter() {
        if value.starts_with('"') {
            // Quoted text is never an item name
            continue;
        }
        if item_parts.iter().all(|part| value.contains(part)) {
            possible_tags.push((tag, value));
        }
    }
    if possible_tags.is_empty() {
        eprintln!("No matching items found");
        std::process::exit(0);
    } else if possible_tags.len() > 1 {
        if let Some(exact_match) = possible_tags.iter().find(|(_, v)| **v == item) {
            possible_tags = vec![*exact_match];
        } else {
            possible_tags.sort_by_key(|(_, v)| *v);
            println!("Multiple tags found, please disambiguate:");
            for (_, value) in possible_tags.iter() {
                println!("  {value}");
            }
            std::process::exit(0);
        }
    }
    let (tag, name) = possible_tags.pop().expect("possible_tags.len() == 1");
    let tag = DatabaseValue::String(tag.to_string());
    let records = iter_records(arz, |id, _raw| id.starts_with("records/items"))
        .filter(|record| record.data.get("itemNameTag") == Some(&tag))
        .map(|record| (record.id.clone(), record))
        .collect::<HashMap<_, _>>();

    (name.to_string(), records)
}
