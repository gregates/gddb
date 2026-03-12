use std::ffi::OsString;
use std::io::{BufRead, Seek};
use std::path::PathBuf;

use lib_gddb::arz::Database;

use crate::util::{
    get_record,
    iter_record_ids,
    list_children,
};

pub fn main<T: BufRead + Seek>(arz: &mut [Database<T>], record: Option<OsString>) {
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
    let ids = iter_record_ids(arz);
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
