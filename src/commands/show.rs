use std::ffi::OsString;
use std::path::PathBuf;

use crate::database::Database;
use crate::util::list_children;

pub fn main(db: &mut Database, record: Option<OsString>) {
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
