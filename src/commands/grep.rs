use std::ffi::OsString;
use std::io::IsTerminal;

use lib_gddb::arc::Archive;
use regex::{Regex, RegexBuilder, escape};

use crate::database::Database;
use crate::{Language, Source};
use crate::util::{path_to, text_resource_paths};

// git grep-like colors, emitted only when writing to a terminal.
const EXPAC: &str = "\x1b[31m"; // red
const PATH: &str = "\x1b[35m"; // magenta
const LINE_NO: &str = "\x1b[33m"; // yellow
const SEP: &str = "\x1b[36m"; // cyan
const MATCH: &str = "\x1b[1;32m"; // bold green
const RESET: &str = "\x1b[0m";

/// Searches every database record and text resource line by line, printing each
/// match as `[gdx<n>:]path:line_number:line`, much like `git grep`.
///
/// Database (`.arz`) records are key/value pairs whose entries are kept in the
/// order they appear in the record, so line numbers are stable across runs.
/// Text resource (`.arc`) files are searched as the line-oriented text they
/// already are; every language is searched unless `language` restricts it to
/// one.
///
/// Matches from an expansion are prefixed with a red `gdx<n>:` segment (base
/// game records have no prefix), kept separate from the path so it lines up with
/// `show` and helps distinguish a base record from an expansion's override.
///
/// With `ignore_case`, matching is case-insensitive; with `fixed_strings`, the
/// pattern is matched literally rather than as a regex. `source` selects which
/// corpora are searched: database records, text resources, or both.
pub fn main(
    db: &mut Database,
    pattern: OsString,
    ignore_case: bool,
    fixed_strings: bool,
    language: Option<Language>,
    source: Source,
) {
    let pattern = pattern.to_string_lossy();
    let needle = if fixed_strings {
        escape(&pattern)
    } else {
        pattern.into_owned()
    };
    let re = match RegexBuilder::new(&needle)
        .case_insensitive(ignore_case)
        .build()
    {
        Ok(re) => re,
        Err(e) => {
            eprintln!("invalid pattern `{needle}`: {e}");
            std::process::exit(1);
        }
    };

    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();

    if matches!(source, Source::Both | Source::Database) {
        grep_databases(db, &re, color);
    }
    if matches!(source, Source::Both | Source::Text) {
        grep_text_resources(&re, color, language);
    }
}

/// Searches resolved `.arz` records. Each `key=value` entry is one line.
fn grep_databases(db: &mut Database, re: &Regex, color: bool) {
    for (xpac, record) in db.iter_records_with_xpac(|_, _| true) {
        for (i, (key, val)) in record.data.iter().enumerate() {
            let line = format!("{key}={val}");
            if re.is_match(&line) {
                print_match(color, xpac, re, &record.id, i + 1, &line);
            }
        }
    }
}

/// Searches the text inside every text resource `.arc` file. Records whose data
/// is not valid UTF-8 are skipped.
fn grep_text_resources(re: &Regex, color: bool, language: Option<Language>) {
    for rel in text_resource_paths(language) {
        let (xpac, display) = split_resource_xpac(&rel);
        let Ok(mut arc) = Archive::open(path_to(&rel)) else {
            continue;
        };
        let Ok(records) = arc.iter_records() else {
            continue;
        };
        for record in records.flatten() {
            let Ok(text) = std::str::from_utf8(&record.data) else {
                continue;
            };
            let path = format!("{display}/{}", record.id);
            for (i, line) in text.lines().enumerate() {
                if re.is_match(line) {
                    print_match(color, xpac, re, &path, i + 1, line);
                }
            }
        }
    }
}

/// Prints a `[gdx<n>:]path:line_number:line` match, optionally colorized with the
/// matched substrings highlighted.
fn print_match(color: bool, xpac: usize, re: &Regex, path: &str, line_no: usize, line: &str) {
    let expac = expac_prefix(color, xpac);
    if !color {
        println!("{expac}{path}:{line_no}:{line}");
        return;
    }
    println!(
        "{expac}{PATH}{path}{RESET}{SEP}:{RESET}{LINE_NO}{line_no}{RESET}{SEP}:{RESET}{}",
        highlight(re, line),
    );
}

/// The leading `gdx<n>:` segment for an expansion (red when colorized), or empty
/// for the base game.
fn expac_prefix(color: bool, xpac: usize) -> String {
    let Some(label) = expac_label(xpac) else {
        return String::new();
    };
    if color {
        format!("{EXPAC}{label}{RESET}{SEP}:{RESET}")
    } else {
        format!("{label}:")
    }
}

/// The label for an expansion, or `None` for the base game (xpac 0).
fn expac_label(xpac: usize) -> Option<&'static str> {
    match xpac {
        1 => Some("gdx1"),
        2 => Some("gdx2"),
        3 => Some("gdx3"),
        _ => None,
    }
}

/// Splits a text resource path into its expansion (xpac) and the display path
/// with the `gdx<n>/` prefix removed: `gdx2/resources/Text_EN.arc` ->
/// `(2, "resources/Text_EN.arc")`. Base-game paths are returned unchanged as
/// xpac 0. The xpac is shown as its own segment, so it is dropped from the path
/// itself for consistency with database records.
fn split_resource_xpac(rel: &str) -> (usize, &str) {
    let Some(rest) = rel.strip_prefix("gdx") else {
        return (0, rel);
    };
    match rest.split_once('/') {
        Some((digits, path)) => match digits.parse::<usize>() {
            Ok(xpac) => (xpac, path),
            Err(_) => (0, rel),
        },
        None => (0, rel),
    }
}

/// Renders `line` with each match wrapped in the match color.
fn highlight(re: &Regex, line: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for m in re.find_iter(line) {
        out.push_str(&line[last..m.start()]);
        out.push_str(MATCH);
        out.push_str(&line[m.start()..m.end()]);
        out.push_str(RESET);
        last = m.end();
    }
    out.push_str(&line[last..]);
    out
}
