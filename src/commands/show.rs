use std::collections::HashSet;
use std::ffi::OsString;
use std::io::{BufRead, Seek};
use std::path::PathBuf;

use lib_gddb::arz::Database;

use crate::util::{
    get_record,
    iter_record_ids,
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
