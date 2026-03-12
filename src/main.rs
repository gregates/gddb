use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::sync::OnceLock;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap::builder::PossibleValue;
use clap_complete::engine::ArgValueCompleter;

use lib_gddb::arz::Database;

mod commands;
mod util;

use crate::util::{
    complete_record_path,
    install_path,
    path_to,
};

const DB_GD: &str = "database/database.arz";
const DB_AOM: &str = "gdx1/database/GDX1.arz";
const DB_FG: &str = "gdx2/database/GDX2.arz";
const DB_FOA: &str = "gdx3/database/GDX3.arz";

const TAG_FILE: &str = "resources/Text_";
const TAG_EXT: &str = ".arc";

const GAME_RANDOMIZER_WEIGHTS: &str = "records/game/gamerandomizerweights.dbr";
const CHALLENGE_LAYER_EASY: &str = "records/game/challengeareas/challengelayer_easy.dbr";
const CHALLENGE_LAYER_HARD: &str = "records/game/challengeareas/challengelayer_hard.dbr";
const CHALLENGE_LAYER_ROGUELIKE: &str = "records/game/challengeareas/challengelayer_hard.dbr";
const CHALLENGE_LAYER_ENDLESS: &str = "records/game/challengeareas/challengelayer_endlessdungeontreasureroom.dbr";

static LANGUAGE: OnceLock<Language> = OnceLock::new();

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, arg_required_else_help = true)]
struct Args {
    #[arg(long, value_name = "SHELL")]
    /// Print shell completion setup instructions
    completions: Option<Shell>,

    #[arg(short, long, default_value_t, ignore_case = true)]
    language: Language,

    #[arg(short, long)]
    /// Restrict lookup to database for nth expansion (0 = base game)
    xpac: Option<usize>,

    #[command(subcommand)]
    cmd: Option<Action>,
}

#[derive(Subcommand, Debug)]
enum Action {
    /// Generate csv for forum rowzero sheet import.
    Csv,
    /// Look up an item by name and list the records it appears in.
    Item { name: OsString },
    /// Show a fully resolved loot table.
    Loot {
        #[arg(short, long, default_value_t, value_enum)]
        challenge: ChallengeLayer,
        #[arg(short, long, default_value_t, value_enum)]
        difficulty: Difficulty,
        #[arg(short, long, default_value_t, value_enum)]
        enemy: MobClass,
        #[arg(short, long, default_value_t, value_enum)]
        affix: AffixesToShow,
        #[arg(short = 'b', long, default_value_t)]
        /// Use chest modifiers, e.g., BossChest for enemy=Boss.
        /// Always true in crucible or sr.
        chest: bool,
        #[arg(short, long, default_value_t)]
        /// Show vendor affix tables (no modifiers). Overrides difficulty, dropper, and challenge.
        vendor: bool,
        #[arg(short, long, default_value_t)]
        /// Include theoretically possible drops whose modified probability is zero.
        zero: bool,
        path_or_item_name: OsString,
    },
    /// Print the specified database record, or list the file tree at the path specified.
    Show {
        #[arg(add = ArgValueCompleter::new(complete_record_path))]
        path: Option<OsString>,
    },
    /// Resolve a tag to localized text.
    Tag { tag: OsString },
}

fn main() {
    clap_complete::CompleteEnv::with_factory(Args::command).complete();
    let args = Args::parse();

    if let Some(shell) = args.completions {
        print_completions(shell);
        return;
    }

    let Some(cmd) = args.cmd else {
        Args::command().print_help().ok();
        std::process::exit(0);
    };

    LANGUAGE
        .set(args.language)
        .expect("LANGUAGE initialized twice");

    let mut dbs = open_dbs(args.xpac);

    match cmd {
        Action::Csv => commands::csv(dbs.as_mut_slice()),
        Action::Loot {
            path_or_item_name,
            difficulty,
            enemy,
            challenge,
            chest,
            affix,
            vendor,
            zero,
            ..
        } => commands::loot(
            dbs.as_mut_slice(),
            path_or_item_name,
            difficulty.into(),
            enemy.into(),
            challenge,
            chest,
            affix,
            vendor,
            zero,
        ),
        Action::Item { name } => commands::item(dbs.as_mut_slice(), name),
        Action::Show { path } => commands::show(dbs.as_mut_slice(), path),
        Action::Tag { tag } => commands::tag(tag.to_string_lossy()),
    }
}

fn print_completions(shell: Shell) {
    let bin = std::env::args().next().unwrap_or_else(|| "gddb".to_string());
    let bin = std::path::Path::new(&bin)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gddb".to_string());

    match shell {
        Shell::Bash => {
            println!("# Add this to your ~/.bashrc:");
            println!("source <(COMPLETE=bash {bin})");
        }
        Shell::Zsh => {
            println!("# Add this to your ~/.zshrc:");
            println!("source <(COMPLETE=zsh {bin})");
        }
        Shell::Fish => {
            println!("# Add this to your ~/.config/fish/config.fish:");
            println!("COMPLETE=fish {bin} | source");
        }
    }
}

fn open_dbs(xpac: Option<usize>) -> Vec<Database<BufReader<File>>> {
    let dbs = match xpac {
        Some(0) => vec![DB_GD],
        Some(1) => vec![DB_AOM],
        Some(2) => vec![DB_FG],
        Some(3) => vec![DB_FOA],
        None => vec![DB_GD, DB_AOM, DB_FG, DB_FOA],
        _ => {
            eprintln!("xpac must be 0, 1, 2, or 3");
            std::process::exit(1)
        }
    }
    .iter()
    .filter_map(|path| Database::open(&path_to(path)).ok())
    .collect::<Vec<_>>();

    if dbs.is_empty() {
        eprintln!(
            "Could not read database files. Please verify GRIM_DAWN_INSTALL_PATH: {}",
            install_path().display(),
        );
        std::process::exit(1);
    }

    dbs
}

#[derive(Default, Debug, Clone, Copy, ValueEnum)]
enum Difficulty {
    Normal,
    Elite,
    #[default]
    Ultimate,
}

impl From<Difficulty> for lib_gddb::Difficulty {
    fn from(difficulty: Difficulty) -> Self {
        match difficulty {
            Difficulty::Normal => Self::Normal,
            Difficulty::Elite => Self::Elite,
            Difficulty::Ultimate => Self::Ultimate,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, ValueEnum)]
enum MobClass {
    Common,
    Champion,
    Hero,
    #[default]
    Boss,
}

impl From<MobClass> for lib_gddb::MobClass {
    fn from(mob: MobClass) -> Self {
        match mob {
            MobClass::Champion => lib_gddb::MobClass::Champion,
            MobClass::Hero => lib_gddb::MobClass::Hero,
            MobClass::Common => lib_gddb::MobClass::Common,
            MobClass::Boss => lib_gddb::MobClass::Boss,
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
enum ChallengeLayer {
    #[default]
    None,
    Dangerous,
    Treacherous,
    Roguelike,
    ShatteredRealm,
    Crucible,
}

impl ValueEnum for ChallengeLayer {
    fn value_variants<'a>() -> &'a[Self] {
        &[
            Self::None,
            Self::Dangerous,
            Self::Treacherous,
            Self::Roguelike,
            Self::Crucible,
            Self::ShatteredRealm,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::None => PossibleValue::new("none").alias("null").alias("0"),
            Self::Dangerous => PossibleValue::new("dangerous").alias("easy").alias("1"),
            Self::Treacherous => PossibleValue::new("treacherous").alias("hard").alias("2"),
            Self::Roguelike => PossibleValue::new("forbidden").alias("roguelike").alias("rogue-like").alias("dungeon").alias("skeleton-key").alias("skeleton-key-dungeon").alias("roguelike-dungeon").alias("3"),
            Self::Crucible => PossibleValue::new("crucible").alias("cruci").alias("4").alias("4+"),
            Self::ShatteredRealm => PossibleValue::new("sr").alias("shatteredrealm").alias("shattered-realm").alias("endless").alias("endlessdungeon").alias("endlessdungeontreasureroom"),
        })
    }
}

#[derive(Default, Debug, Clone, Copy, ValueEnum)]
enum Language {
    Cs,
    De,
    #[default]
    En,
    Es,
    Fr,
    It,
    Ja,
    Ko,
    Pl,
    Pt,
    Ru,
    Vi,
    Zh,
}

impl Language {
    fn as_str(&self) -> &str {
        match self {
            Self::Cs => "CS",
            Self::De => "DE",
            Self::En => "EN",
            Self::Es => "ES",
            Self::Fr => "FR",
            Self::It => "IT",
            Self::Ja => "JA",
            Self::Ko => "KO",
            Self::Pl => "PL",
            Self::Pt => "PT",
            Self::Ru => "RU",
            Self::Vi => "VI",
            Self::Zh => "ZH",
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Default, Debug, Clone, Copy)]
enum AffixesToShow {
    Prefix,
    Suffix,
    #[default]
    All,
}

impl ValueEnum for AffixesToShow {
    fn value_variants<'a>() -> &'a[Self] {
        &[
            Self::Prefix,
            Self::Suffix,
            Self::All,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::Prefix => PossibleValue::new("prefix").alias("pre").alias("p"),
            Self::Suffix => PossibleValue::new("suffix").alias("suf").alias("s"),
            Self::All => PossibleValue::new("all").alias("both"),
        })
    }
}
