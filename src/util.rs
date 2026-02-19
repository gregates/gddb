use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, Seek};
use std::path::PathBuf;
use std::sync::LazyLock;

use lib_gddb::arc::Archive;
use lib_gddb::arz::{Database, DatabaseValue, RawRecord, Record};
use lib_gddb::tags;

use crate::{
    INSTALL_PATH,
    LANGUAGE,
    Language,
    TAG_FILE,
    TAG_EXT,
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

/// Returns the install path specified for this execution of the program.
pub fn install_path() -> &'static PathBuf {
    INSTALL_PATH.get().unwrap()
}

/// Returns the fully qualified path for a resource file specified relative to install path.
pub fn path_to(path: impl AsRef<str>) -> PathBuf {
    INSTALL_PATH.get().unwrap().join(path.as_ref())
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
            "Could not read tag files. Please verify install path: {}",
            install_path().display()
        );
        std::process::exit(1);
    };

    item_tags
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
