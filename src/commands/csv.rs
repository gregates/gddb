use lib_gddb::affix::Affix;
use lib_gddb::affix_combo_weights::AffixComboModifiers;
use lib_gddb::affix_table::AffixTable;
use lib_gddb::item::Item;
use lib_gddb::loot_table::LootTable;
use lib_gddb::rarity::Rarity;
use lib_gddb::{Difficulty, MobClass};
use std::collections::HashMap;

use crate::GAME_RANDOMIZER_WEIGHTS;
use crate::database::Database;
use crate::util::TAGS;

pub fn main(db: &mut Database) {
    let tags = &*TAGS;
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
    let modifiers = AffixComboModifiers::from(&db.get_record(GAME_RANDOMIZER_WEIGHTS.into()));

    let mut loot_tables: HashMap<String, LootTable> = Default::default();

    for record in db.iter_records(|id, raw| {
        id.starts_with("records/items/loottables/") && raw.kind == "LootItemTable_DynWeight"
    }) {
        if record.id.contains("nemesis") && !record.id.contains("03") {
            // Always use the 3rd nemesis table
            continue;
        }
        if record.id.ends_with("tdyn_weaponstandin_a01.dbr") {
            // References non-existent affix table
            continue;
        }
        if record.id.ends_with("broken.dbr") {
            // Not sure exactly what these are, but I don't think they're useful.
            continue;
        }
        let loot_table = LootTable::from(&record);
        loot_tables.insert(record.id.clone(), loot_table);
    }

    let rare_items = db
        .iter_records(|id, _raw| {
            id.starts_with("records/items/")
                && !id.starts_with("records/items/lore")
                && !id.starts_with("records/items/loot")
                && !id.starts_with("records/items/misc")
                && !id.starts_with("records/items/crafting")
                && !id.starts_with("records/items/enemygear")
        })
        .map(|record| (record.id.clone(), Item::from(&record)))
        .filter(|(_, item)| item.rarity == Rarity::Rare && item.level == 94)
        .collect::<HashMap<_, _>>();

    println!("Loot Table\tPrefix\tItem\tSuffix\tPrefix Tier\tSuffix Tier\tChance");
    for (id, loot_table) in loot_tables {
        let modifiers = modifiers.get(Difficulty::Ultimate.into(), MobClass::Boss.into(), false);
        let mut resolved =
            loot_table.resolve(100u32, &modifiers, &affix_table_lookup, &affix_lookup);
        resolved.sort_by(|(_, _, a), (_, _, b)| a.total_cmp(&b).reverse());
        for loot in loot_table.loots {
            let Some(loot) = rare_items.get(&loot.id) else {
                continue;
            };
            let loot_name = tags.get(&loot.tag).unwrap_or_else(|| &loot.tag);
            for (prefix, suffix, chance) in &resolved {
                let p = prefix
                    .map(|affix| affix.localize(&tags))
                    .unwrap_or_default();
                let s = suffix
                    .map(|affix| affix.localize(&tags))
                    .unwrap_or_default();
                let pr = prefix
                    .map(|affix| affix.rarity.to_string())
                    .unwrap_or("None".to_string());
                let sr = suffix
                    .map(|affix| affix.rarity.to_string())
                    .unwrap_or("None".to_string());
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    id, p, loot_name, s, pr, sr, chance
                );
            }
        }
    }
}
