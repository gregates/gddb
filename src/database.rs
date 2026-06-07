use lib_gddb::arz::{Database as Arz, DatabaseValue, RawRecord, Record};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;

use crate::util::{TAGS, path_to, xpac_db_path};

#[derive(Default)]
pub struct Database {
    arz: [Option<Arz<BufReader<File>>>; 4],
}

impl Database {
    /// Opens database files for base game and all expansions.
    /// Fails if base game database is not present, but will just ignore any xpacs that aren't.
    pub fn load_all() -> Self {
        // Base game must open.
        let mut db = Self::load_one(0);
        for i in 1..=3 {
            let _ = db.try_open(i);
        }
        db
    }

    // Opens the database file for the specific xpac. Fails if not present.
    pub fn load_one(xpac: usize) -> Self {
        let mut db = Self::default();
        db.open(xpac);
        db
    }

    fn try_open(&mut self, xpac: usize) -> std::io::Result<()> {
        let path = xpac_db_path(xpac);
        let arz = Arz::open(&path_to(path))?;
        self.arz[xpac] = Some(arz);
        Ok(())
    }

    fn open(&mut self, xpac: usize) {
        if let Err(e) = self.try_open(xpac) {
            eprintln!("Could not read database files. Please verify GRIM_DAWN_INSTALL_PATH: {e}");
            std::process::exit(1);
        }
    }

    /// Gets a specific record from any of the provided databases by id.
    /// If more than one record with that id exists, the last one is
    /// returned.
    pub fn get_record(&mut self, matches: OsString) -> Record {
        let needle = matches.to_string_lossy();
        let mut matches = self.iter_records(|id, _| id == needle).collect::<Vec<_>>();
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

    /// Yields all records that satisfy predicate p from the provided
    /// databases.
    pub fn iter_records(
        &mut self,
        p: impl Fn(&str, &RawRecord) -> bool,
    ) -> impl Iterator<Item = Record> + '_ {
        self.iter_records_with_xpac(p).map(|(_, record)| record)
    }

    /// Like [`iter_records`](Self::iter_records), but pairs each record with the
    /// expansion (xpac) of the database it came from (0 = base game).
    pub fn iter_records_with_xpac(
        &mut self,
        p: impl Fn(&str, &RawRecord) -> bool,
    ) -> impl Iterator<Item = (usize, Record)> + '_ {
        self.records_by_xpac(p)
            .into_iter()
            .enumerate()
            .flat_map(|(xpac, records)| records.into_iter().map(move |record| (xpac, record)))
    }

    /// Yields all record ids from the provided databases.
    pub fn iter_record_ids(&mut self) -> impl Iterator<Item = String> + '_ {
        match self
            .load_raws_by_xpac()
            .into_iter()
            .zip(self.iter_mut())
            .map(|(raws, arz)| {
                arz.as_mut()
                    .map(|arz| {
                        raws.into_iter()
                            .map(|raw| arz.record_id(&raw))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap_or_else(|| Ok(vec![]))
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

    fn records_by_xpac(&mut self, p: impl Fn(&str, &RawRecord) -> bool) -> Vec<Vec<Record>> {
        match self
            .load_raws_by_xpac()
            .into_iter()
            .zip(self.iter_mut())
            .map(|(raws, arz)| {
                arz.as_mut()
                    .map(|arz| {
                        raws.into_iter()
                            .filter_map(|raw| {
                                let id = arz.record_id(&raw).ok()?;
                                if p(id.as_str(), &raw) {
                                    Some(arz.resolve(raw))
                                } else {
                                    None
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap_or_else(|| Ok(vec![]))
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

    fn load_raws_by_xpac(&mut self) -> Vec<Vec<RawRecord>> {
        self.iter_mut()
            .map(|db| {
                db.as_mut()
                    .map(|db| {
                        db.iter_records()
                            .unwrap()
                            .map(|result| result.unwrap())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut Option<Arz<BufReader<File>>>> + use<'_> {
        self.arz.iter_mut()
    }

    /// Attempts to find the set of records for an item by item name.
    /// If a partial item name is given that matches more than one tag, the matches are printed and the program exits.
    /// If the partial item name matches a unique tag, that tag is returned along with the set of records that reference it.
    pub fn lookup_item(&mut self, item: OsString) -> (String, HashMap<String, Record>) {
        let item = item.to_string_lossy();
        let item_parts = item.split_ascii_whitespace().collect::<Vec<_>>();
        let tags = &*TAGS;
        let mut possible_tags = vec![];
        for (tag, value) in tags.iter() {
            if value.starts_with('"') {
                // Quoted text is never an item name
                continue;
            }
            if item_parts
                .iter()
                .all(|part| value.to_lowercase().contains(&part.to_lowercase()))
            {
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
        let records = self
            .iter_records(|id, _raw| id.starts_with("records/items"))
            .filter(|record| record.data.get("itemNameTag") == Some(&tag))
            .map(|record| (record.id.clone(), record))
            .collect::<HashMap<_, _>>();

        (name.to_string(), records)
    }
}
