use std::ffi::OsString;

use crate::database::Database;

pub fn main(db: &mut Database, item: OsString) {
    let (name, records) = db.lookup_item(item);
    println!("{name} is referenced in the following database records:");
    for (id, _record) in records {
        println!("  {id}");
    }
}
