use std::ffi::OsString;
use std::path::PathBuf;

use crate::database::Database;
use crate::util::list_children;

pub fn main(db: &mut Database, record: Option<OsString>, all: bool) {
    if all {
        list_all(db);
        return;
    }
    let record = record.unwrap_or("".into());
    if PathBuf::from(record.clone())
        .extension()
        .map(|ext| ext.to_str())
        .flatten()
        == Some("dbr")
    {
        print!("{}", db.get_record(record));
    } else {
        ls(db, Some(record));
    }
}

/// Prints the full path of every record, one per line, deduplicated (a record
/// can exist in both the base game and an expansion) and sorted.
fn list_all(db: &mut Database) {
    let mut ids = db.iter_record_ids().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    for id in ids {
        println!("{id}");
    }
}

fn ls(db: &mut Database, prefix: Option<OsString>) {
    let ids = db.iter_record_ids();
    let children = list_children(ids, prefix.as_deref());
    if children.is_empty() {
        eprintln!(
            "No database records match prefix {}",
            PathBuf::from(prefix.unwrap_or("/".into())).display()
        );
    } else {
        for path in children {
            println!("{path}");
        }
    }
}
