use lib_gddb::arc::Archive;
use lib_gddb::tags;

use crate::util::{
    path_to,
    text_resource_path,
};

pub fn main(tag: impl AsRef<str> + std::fmt::Display) {
    let mut values = vec![];
    for (xpac, mut arc) in (0..=3)
        .into_iter()
        .map(|xpac| (xpac, path_to(text_resource_path(xpac))))
        .filter_map(|(xpac, path)| Archive::open(&path).ok().map(|arc| (xpac, arc)))
    {
        for record in arc.iter_records().unwrap().filter(|record| {
            record
                .as_ref()
                .ok()
                .filter(|record| record.id.contains("tag"))
                .is_some()
        }) {
            let record = record.unwrap();
            let tags = tags::parse(&record.data);
            match tags {
                Ok(tags) => {
                    if let Some(text) = tags.get(tag.as_ref()).cloned() {
                        values.push((xpac, record.id, text));
                    }
                }
                Err(_e) => {} // eprintln!("failed to parse tags for {}: {:#?}", record.id, _e),
            }
        }
    }
    values.dedup_by(|(_, _, a), (_, _, b)| a == b);
    if values.len() == 1 {
        println!("{}", values[0].2);
    } else if values.len() > 1 {
        println!("Multiple tag values found:");
        for (xpac, id, text) in values {
            println!("{}/{} maps {} to {}", text_resource_path(xpac), id, tag, text);
        }
    } else {
        eprintln!("Tag not found");
    }
}
