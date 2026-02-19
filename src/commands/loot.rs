use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, Seek};

use lib_gddb::{Difficulty, MobClass};
use lib_gddb::affix::Affix;
use lib_gddb::affix_combo_weights::{AffixComboModifiers, AffixComboWeights};
use lib_gddb::affix_table::AffixTable;
use lib_gddb::arz::{Database, Record};
use lib_gddb::loot_table::LootTable;

use crate::{
    ChallengeLayer,
    CHALLENGE_LAYER_EASY,
    CHALLENGE_LAYER_HARD,
    CHALLENGE_LAYER_ROGUELIKE,
    CHALLENGE_LAYER_ENDLESS,
    GAME_RANDOMIZER_WEIGHTS,
};
use crate::util::{
    get_record,
    iter_records,
    lookup_item,
    TAGS,
};

const ZERO_THRESHHOLD: f64 = 0.0000001f64;

pub fn main<T: BufRead + Seek>(
    arz: &mut [Database<T>],
    record: OsString,
    difficulty: Difficulty,
    class: MobClass,
    challenge: ChallengeLayer,
    is_chest: bool,
    prefix: bool,
    suffix: bool,
    vendor: bool,
    zero: bool,
) {
    let tags = &*TAGS;
    let loot_table = resolve_loot_table(arz, record);
    let loot_table = LootTable::from(&loot_table);
    let affixes = iter_records(arz, |_, raw| raw.kind == "LootRandomizer")
        .map(|record| Affix::from(record))
        .collect::<Vec<_>>();
    let affix_lookup = affixes
        .iter()
        .map(|affix| (affix.id.clone(), affix))
        .collect::<HashMap<_, _>>();
    let affix_tables = iter_records(arz, |_, raw| raw.kind == "LootRandomizerTable")
        .map(|record| AffixTable::from(&record))
        .collect::<Vec<_>>();
    let affix_table_lookup = affix_tables
        .into_iter()
        .map(|table| (table.id.clone(), table))
        .collect::<HashMap<_, _>>();
    let modifier_record = match challenge {
        ChallengeLayer::None => GAME_RANDOMIZER_WEIGHTS,
        ChallengeLayer::Dangerous => CHALLENGE_LAYER_EASY,
        ChallengeLayer::Treacherous => CHALLENGE_LAYER_HARD,
        ChallengeLayer::Roguelike => CHALLENGE_LAYER_ROGUELIKE,
        ChallengeLayer::Crucible | ChallengeLayer::ShatteredRealm => CHALLENGE_LAYER_ENDLESS,
    };
    let modifiers = AffixComboModifiers::from(&get_record(arz, modifier_record.into()));
    let modifiers = if vendor {
        AffixComboWeights::default()
    } else {
        let is_chest = is_chest || match challenge {
            ChallengeLayer::Crucible | ChallengeLayer::ShatteredRealm => true,
            _ => false,
        };
        modifiers.get(difficulty, class, is_chest)
    };

    if prefix {
        let mut resolved =
            loot_table.resolve_prefix(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
        resolved.sort_by(|(_, a), (_, b)| a.total_cmp(&b).reverse());
        for (prefix, chance) in resolved {
            if !zero && chance < ZERO_THRESHHOLD {
                continue;
            }
            print!("{:0.08}%\t", chance * 100f64);
            let prefix = prefix.map(|p| p.localize(tags)).unwrap_or_default();
            println!("{prefix}");
        }
        return;
    }

    if suffix {
        let mut resolved =
            loot_table.resolve_suffix(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
        resolved.sort_by(|(_, a), (_, b)| a.total_cmp(&b).reverse());
        for (suffix, chance) in resolved {
            if !zero && chance < ZERO_THRESHHOLD {
                continue;
            }
            print!("{:0.08}%\t", chance * 100f64);
            let suffix = suffix.map(|s| s.localize(tags)).unwrap_or_default();
            println!("{suffix}");
        }
        return;
    }

    let mut resolved = loot_table.resolve(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
    resolved.sort_by(|(_, _, a), (_, _, b)| a.total_cmp(&b).reverse());
    for (prefix, suffix, chance) in resolved {
        if !zero && chance < ZERO_THRESHHOLD {
            continue;
        }
        print!("{:0.08}%\t", chance * 100f64);
        let prefix = prefix.map(|p| p.localize(tags)).unwrap_or_default();
        print!("{prefix}\t");
        let tabs = 2 - prefix.len() / 8;
        for _ in 0..tabs {
            print!("\t");
        }
        let suffix = suffix.map(|s| s.localize(tags)).unwrap_or_default();
        println!("{suffix}");
    }
}

fn resolve_loot_table<T: BufRead + Seek>(arz: &mut [Database<T>], record: OsString) -> Record {

    if is_path(&record) {
        get_record(arz, record)
    } else {
        let (name, records) = lookup_item(arz, record);
        let Some((item_id, _item)) = records.into_iter().max_by_key(|(_id, record)| record.data["itemLevel"].as_int().unwrap_or(0)) else {
            eprintln!("No matching items found");
            std::process::exit(0);
        };
        let mut loot_tables = iter_records(arz, |_, raw| raw.kind == "LootItemTable_DynWeight")
            .filter(|record| record.data.iter().any(|(_, val)| val.as_string().as_ref() == Some(&item_id)))
            .collect::<Vec<_>>();
        if loot_tables.len() > 1 {
            eprintln!("WARNING: Found multiple loot tables for {name}; using last table in this list:");
            for table in loot_tables.iter() {
                eprintln!("  {}", table.id);
            }
        }
        let Some(table) = loot_tables.pop() else {
            eprintln!("No loot table found for {name}");
            std::process::exit(0);
        };
        table
    }
}

fn is_path(maybe_path: &OsString) -> bool {
    maybe_path.to_string_lossy().ends_with(".dbr")
}


