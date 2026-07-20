use std::collections::HashMap;
use std::ffi::OsString;

use lib_gddb::affix::Affix;
use lib_gddb::affix_combo_weights::{AffixComboModifiers, AffixComboWeights};
use lib_gddb::affix_table::AffixTable;
use lib_gddb::arz::Record;
use lib_gddb::ascension_affix_swap::AscensionAffixSwap;
use lib_gddb::loot_table::LootTable;
use lib_gddb::{Difficulty, MobClass};

use crate::database::Database;
use crate::util::TAGS;
use crate::{
    AffixAffinity, AffixesToShow, CHALLENGE_LAYER_EASY, CHALLENGE_LAYER_ENDLESS,
    CHALLENGE_LAYER_HARD, CHALLENGE_LAYER_ROGUELIKE, ChallengeLayer, GAME_RANDOMIZER_WEIGHTS,
};

const ASCENSION_AFFIX_SWAP_LISTS: &str = "records/items/lootaffixes/ascensionaffixswaplists";
const ENDLESS_DUNGEON_LOOT_TABLES: &str = "records/endlessdungeon/loottables/";

const ZERO_THRESHHOLD: f64 = 0.00000000001f64;

pub fn main(
    db: &mut Database,
    record: OsString,
    difficulty: Difficulty,
    class: MobClass,
    challenge: ChallengeLayer,
    is_chest: bool,
    affixes_to_show: AffixesToShow,
    vendor: bool,
    zero: bool,
    altar_swap: Option<AffixAffinity>,
) {
    let tags = &*TAGS;
    let loot_table = resolve_loot_table(db, record);
    let mut loot_table = LootTable::from(&loot_table);
    if let Some(affinity) = altar_swap {
        let prefix = format!("{ASCENSION_AFFIX_SWAP_LISTS}/{}/", affinity.as_str());
        let swaps = db
            .iter_records(|id, _| id.starts_with(&prefix))
            .map(|record| AscensionAffixSwap::from(&record))
            .collect::<Vec<_>>();
        loot_table.apply_affix_swaps(&swaps);
    }
    let affixes = db
        .iter_records(|_, raw| raw.kind == "LootRandomizer")
        .map(|record| Affix::from(record))
        .collect::<Vec<_>>();
    let affix_lookup = affixes
        .iter()
        .map(|affix| (affix.id.clone(), affix))
        .collect::<HashMap<_, _>>();
    let affix_tables = db
        .iter_records(|_, raw| raw.kind == "LootRandomizerTable")
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
    let modifiers = AffixComboModifiers::from(&db.get_record(modifier_record.into()));
    let modifiers = if vendor {
        AffixComboWeights::default()
    } else {
        let is_chest = is_chest
            || match challenge {
                ChallengeLayer::Crucible | ChallengeLayer::ShatteredRealm => true,
                _ => false,
            };
        modifiers.get(difficulty, class, is_chest)
    };

    match affixes_to_show {
        AffixesToShow::Prefix => {
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
        }
        AffixesToShow::Suffix => {
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
        }
        AffixesToShow::All => {
            let mut resolved =
                loot_table.resolve(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
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
    }
}

fn resolve_loot_table(db: &mut Database, record: OsString) -> Record {
    if is_path(&record) {
        db.get_record(record)
    } else {
        let (name, records) = db.lookup_item(record);
        let Some((item_id, _item)) = records
            .into_iter()
            .max_by_key(|(_id, record)| record.data["itemLevel"].as_int().unwrap_or(0))
        else {
            eprintln!("No matching items found");
            std::process::exit(0);
        };
        let mut loot_tables = db
            .iter_records(|id, raw| {
                raw.kind == "LootItemTable_DynWeight"
                    && !id.starts_with(ENDLESS_DUNGEON_LOOT_TABLES)
            })
            .filter(|record| {
                record
                    .data
                    .iter()
                    .any(|(_, val)| val.as_string().as_ref() == Some(&item_id))
            })
            .collect::<Vec<_>>();
        if loot_tables.len() > 1 {
            eprintln!(
                "WARNING: Found multiple loot tables for {name}; using last table in this list:"
            );
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
