use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek};
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use lib_gddb::affix::Affix;
use lib_gddb::affix_combo_weights::{AffixComboModifiers, AffixComboWeights};
use lib_gddb::affix_table::AffixTable;
use lib_gddb::arc::Archive;
use lib_gddb::arz::{Database, DatabaseValue, RawRecord, Record};
use lib_gddb::loot_table::LootTable;
use lib_gddb::tags;

const DB_GD: &str = "database/database.arz";
const DB_AOM: &str = "gdx1/database/GDX1.arz";
const DB_FG: &str = "gdx2/database/GDX2.arz";
const DB_FOA: &str = "gdx3/database/GDX3.arz";

const TAGS_GD: &str = "resources/Text_EN.arc";
const TAGS_AOM: &str = "gdx1/resources/Text_EN.arc";
const TAGS_FG: &str = "gdx2/resources/Text_EN.arc";
const TAGS_FOA: &str = "gdx3/resources/Text_EN.arc";

const GAME_RANDOMIZER_WEIGHTS: &str = "records/game/gamerandomizerweights.dbr";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Args {
    #[arg(short, long)]
    /// Path to Grim Dawn installation
    install_path: Option<OsString>,

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

impl From<Difficulty> for lib_gddb::Difficulty {
    fn from(difficulty: Difficulty) -> Self {
        match difficulty {
            Difficulty::Normal => Self::Normal,
            Difficulty::Elite => Self::Elite,
            Difficulty::Ultimate => Self::Ultimate,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, ValueEnum)]
enum MobClass {
    Common,
    Champion,
    Hero,
    #[default]
    Boss,
}

impl From<MobClass> for lib_gddb::MobClass {
    fn from(mob: MobClass) -> Self {
        match mob {
            MobClass::Champion => lib_gddb::MobClass::Champion,
            MobClass::Hero => lib_gddb::MobClass::Hero,
            MobClass::Common => lib_gddb::MobClass::Common,
            MobClass::Boss => lib_gddb::MobClass::Boss,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Action {
    /// Look up an item by name and list the records it appears in.
    Item { name: OsString },
    /// Show a fully resolved loot table
    LootTable {
        #[arg(short, long, default_value_t, value_enum)]
        difficulty: Difficulty,
        #[arg(short, long, default_value_t, value_enum)]
        dropper: MobClass,
        #[arg(short, long, default_value_t)]
        /// Show vendor affix tables (no modifiers). Overrides difficulty, dropper, and challenge
        /// layer.
        vendor: bool,
        #[arg(short, long, default_value_t)]
        /// Only show possible prefixes; supercedes suffix.
        prefix: bool,
        #[arg(short, long, default_value_t)]
        /// Only show possible suffixes.
        suffix: bool,
        path: OsString,
    },
    /// Print the specified database record, or list the file tree at the path specified.
    Show { path: Option<OsString> },
}

fn main() {
    let args = Args::parse();

    let install_path = args
        .install_path
        .or(env::var("GRIM_DAWN_INSTALL_PATH").ok().map(|s| s.into()))
        .map(|path| PathBuf::from(path))
        .unwrap_or_else(|| {
            eprintln!("Please provide --install-path or set GRIM_DAWN_INSTALL_PATH");
            std::process::exit(1);
        });

    let mut dbs = open_dbs(install_path.clone(), args.xpac);

    let item_tags = read_item_tags(install_path.clone());

    match args.cmd {
        Action::LootTable {
            path, difficulty, dropper, prefix, suffix, vendor, ..
        } => loot_table(dbs.as_mut_slice(), item_tags, path, difficulty, dropper, prefix, suffix, vendor),
        Action::Item { name } => item(dbs.as_mut_slice(), item_tags, name),
        Action::Show { path } => show(dbs.as_mut_slice(), path),
    }
}

fn loot_table<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    tags: HashMap<String, String>,
    record: OsString,
    difficulty: Difficulty,
    dropper: MobClass,
    prefix: bool,
    suffix: bool,
    vendor: bool,
) {
    let loot_table = get_record(arz, record);
    let loot_table = LootTable::from(&loot_table);
    let affixes = iter_records(arz, |_, raw| raw.kind == "LootRandomizer")
        .map(|record| Affix::from(record))
        .collect::<Vec<_>>();
    let affix_lookup = affixes.iter().map(|affix| (affix.id.clone(), affix)).collect::<HashMap<_, _>>();
    let affix_tables = iter_records(arz, |_, raw| raw.kind == "LootRandomizerTable")
        .map(|record| AffixTable::from(&record))
        .collect::<Vec<_>>();
    let affix_table_lookup = affix_tables.into_iter().map(|table| (table.id.clone(), table)).collect::<HashMap<_, _>>();
    let modifiers = AffixComboModifiers::from(&get_record(arz, GAME_RANDOMIZER_WEIGHTS.into()));
    let modifiers = if vendor {
        AffixComboWeights::default()
    } else {
        modifiers.get(difficulty.into(), dropper.into(), false)
    };

    if prefix {
        let mut resolved = loot_table.resolve_prefix(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
        resolved.sort_by(|(_, a), (_, b)| a.total_cmp(&b).reverse());
        for (prefix, chance) in resolved {
            print!("{:0.08}%\t", chance * 100f64);
            let prefix = prefix.map(|p| p.localize(&tags)).unwrap_or_default();
            println!("{prefix}");
        }
        return;
    }

    if suffix {
        let mut resolved = loot_table.resolve_suffix(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
        resolved.sort_by(|(_, a), (_, b)| a.total_cmp(&b).reverse());
        for (suffix, chance) in resolved {
            print!("{:0.08}%\t", chance * 100f64);
            let suffix = suffix.map(|s| s.localize(&tags)).unwrap_or_default();
            println!("{suffix}");
        }
        return;
    }

    let mut resolved = loot_table.resolve(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
    resolved.sort_by(|(_, _, a), (_, _, b)| a.total_cmp(&b).reverse());
    for (prefix, suffix, chance) in resolved {
        print!("{:0.08}%\t", chance * 100f64);
        let prefix = prefix.map(|p| p.localize(&tags)).unwrap_or_default();
        print!("{prefix}\t");
        let tabs = 2 - prefix.len() / 8;
        for _ in 0..tabs {
            print!("\t");
        }
        let suffix = suffix.map(|s| s.localize(&tags)).unwrap_or_default();
        println!("{suffix}");
    }
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

fn show<T: BufRead + Seek>(arz: &mut [Database<T>], record: Option<OsString>) {
    let record = record.unwrap_or("".into());
    if PathBuf::from(record.clone())
        .extension()
        .map(|ext| ext.to_str())
        .flatten()
        == Some("dbr")
    {
        print!("{}", get_record(arz, record));
    } else {
        ls(arz, Some(record));
    }
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
    if nexts.is_empty() {
        eprintln!(
            "No database records match prefix {}",
            PathBuf::from(prefix.unwrap_or("/".into())).display()
        );
    } else {
        let mut sorted = Vec::with_capacity(nexts.len());
        for path in nexts {
            sorted.push(path);
        }
        sorted.sort();
        for path in sorted {
            println!("{path}");
        }
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
