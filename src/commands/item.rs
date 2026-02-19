use std::io::{BufRead, Seek};
use std::ffi::OsString;

use lib_gddb::arz::Database;

use crate::util::lookup_item;

pub fn main<T: BufRead + Seek>(arz: &mut [Database<T>], item: OsString) {
    let (name, records) = lookup_item(arz, item);
    println!("{name} is referenced in the following database records:");
    for (id, _record) in records {
        println!("  {id}");
    }
}
