use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use lib_gddb::arc::Archive;
use lib_gddb::arz::{Database, DatabaseValue, RawRecord, Record};
use lib_gddb::tags;

const DB_GD: &str = "database/database.arz";
const DB_AOM: &str = "gdx1/database/GDX1.arz";
const DB_FG: &str = "gdx2/database/GDX2.arz";
const DB_FOA: &str = "gdx3/database/GDX3.arz";

const TAGS_GD: &str = "resources/Text_EN.arc";
const TAGS_AOM: &str = "gdx1/resources/Text_EN.arc";
const TAGS_FG: &str = "gdx2/resources/Text_EN.arc";
const TAGS_FOA: &str = "gdx3/resources/Text_EN.arc";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Args {
    #[arg(short, long)]
    /// Path to Grim Dawn installation
    install_path: OsString,

    #[arg(short, long)]
    /// Restrict lookup to database for nth expansion (0 = base game)
    xpac: Option<usize>,

    #[command(subcommand)]
    cmd: Action,
}

#[derive(Default, Debug, Clone, Copy, ValueEnum)]
enum Difficulty {
    Normal,
    Elite,
    #[default]
    Ultimate,
}

#[derive(Subcommand, Debug)]
enum Action {
    /// Show a fully resolved loot table
    LootTable {
        #[arg(short, long, default_value_t, value_enum)]
        difficulty: Difficulty,
        #[arg(short, long, default_value_t)]
        /// Show vendor affix tables (no modifiers). Overrides difficulty, dropper, and challenge
        /// layer.
        vendor: bool,
        path: OsString,
    },
    /// Look up an item by name and list the records it appears in.
    Item { name: OsString },
    /// Show the next level of the file tree, starting at the provided path.
    Ls { path: Option<OsString> },
    /// Print the specified database record.
    Show { path: OsString },
}

fn main() {
    let args = Args::parse();

    let install_path = PathBuf::from(args.install_path);

    let mut dbs = open_dbs(install_path.clone(), args.xpac);

    let item_tags = read_item_tags(install_path.clone());

    match args.cmd {
        Action::LootTable {
            path, difficulty, ..
        } => loot_table(dbs.as_mut_slice(), item_tags, path, difficulty),
        Action::Item { name } => item(dbs.as_mut_slice(), item_tags, name),
        Action::Ls { path } => ls(dbs.as_mut_slice(), path),
        Action::Show { path } => show(dbs.as_mut_slice(), path),
    }
}

fn loot_table<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    tags: HashMap<String, String>,
    record: OsString,
    difficulty: Difficulty,
) {
    let loot_table = get_record(arz, record);
    let affixes = iter_records(arz, |_, raw| raw.kind == "lootRandomizer");
    print!("{loot_table}");
}

fn item<T: BufRead + Seek>(arz: &mut [Database<T>], tags: HashMap<String, String>, item: OsString) {
    let (name, ids) = lookup_item_ids(arz, &tags, item);
    println!("{name} is referenced in the following database records:");
    for record in ids {
        println!("  {record}");
    }
}

fn lookup_item_ids<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    tags: &HashMap<String, String>,
    item: OsString,
) -> (String, HashSet<String>) {
    let item = item.to_string_lossy();
    let item_parts = item.split_ascii_whitespace().collect::<Vec<_>>();
    let mut possible_tags = vec![];
    for (tag, value) in tags.iter() {
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
            println!("Multiple item tags found, please disambiguate:");
            for (_, value) in possible_tags.iter() {
                println!("  {value}");
            }
            std::process::exit(0);
        }
    }
    let (tag, name) = possible_tags.pop().expect("possible_tags.len() == 1");
    let tag = DatabaseValue::String(tag.to_string());
    let ids = iter_records(arz, |id, _raw| id.starts_with("records/items"))
        .filter(|record| record.data.get("itemNameTag") == Some(&tag))
        .map(|record| record.id)
        .collect::<HashSet<_>>();

    (name.to_string(), ids)
}

fn get_record<T: BufRead + Seek>(arz: &mut [Database<T>], matches: OsString) -> Record {
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

fn show<T: BufRead + Seek>(arz: &mut [Database<T>], record: OsString) {
    print!("{}", get_record(arz, record));
}

fn ls<T: BufRead + Seek>(arz: &mut [Database<T>], prefix: Option<OsString>) {
    let nexts = iter_record_ids(arz)
        .filter_map(|id| {
            let path = PathBuf::from(&id);
            let path = match &prefix {
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
        .collect::<HashSet<_>>();
    let mut sorted = Vec::with_capacity(nexts.len());
    for path in nexts {
        sorted.push(path);
    }
    sorted.sort();
    for path in sorted {
        println!("{path}");
    }
}

fn iter_records<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    p: impl Fn(&str, &RawRecord) -> bool,
) -> impl Iterator<Item = Record> + '_ {
    records_by_xpac(arz, p)
        .into_iter()
        .map(|db| db.into_iter())
        .flatten()
}

fn iter_record_ids<T: BufRead + Seek>(
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

fn open_dbs(install_path: PathBuf, xpac: Option<usize>) -> Vec<Database<BufReader<File>>> {
    let dbs = match xpac {
        Some(0) => vec![DB_GD],
        Some(1) => vec![DB_AOM],
        Some(2) => vec![DB_FG],
        Some(3) => vec![DB_FOA],
        None => vec![DB_GD, DB_AOM, DB_FG, DB_FOA],
        _ => {
            eprintln!("xpac must be 0, 1, 2, or 3");
            std::process::exit(1)
        }
    }
    .iter()
    .map(|path| install_path.join(path))
    .filter_map(|path| Database::open(&path).ok())
    .collect::<Vec<_>>();

    if dbs.is_empty() {
        eprintln!(
            "Could not read database files. Please verify install path: {}",
            install_path.display(),
        );
        std::process::exit(1);
    }

    dbs
}

fn read_item_tags(install_path: PathBuf) -> HashMap<String, String> {
    let item_tags = [TAGS_GD, TAGS_AOM, TAGS_FG, TAGS_FOA]
        .iter()
        .map(|path| install_path.join(path))
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
            install_path.display()
        );
        std::process::exit(1);
    };

    item_tags
}
