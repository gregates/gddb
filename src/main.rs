use std::ffi::OsString;
use std::sync::OnceLock;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap::builder::PossibleValue;
use clap_complete::engine::ArgValueCompleter;

mod commands;
mod database;
mod util;

use crate::database::Database;
use crate::util::{
    complete_record_path,
    install_path,
};

const DB_GD: &str = "database/database.arz";
const DB_AOM: &str = "gdx1/database/GDX1.arz";
const DB_FG: &str = "gdx2/database/GDX2.arz";
const DB_FOA: &str = "gdx3/database/GDX3.arz";

const TAG_DIR: &str = "resources";
const TAG_FILE_PREFIX: &str = "Text_";
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
    /// Search all database records and text resources for lines matching a regex.
    Grep {
        #[arg(short = 'i', long)]
        /// Case-insensitive matching.
        ignore_case: bool,
        #[arg(short = 'F', long)]
        /// Match the pattern as a literal string instead of a regex.
        fixed_strings: bool,
        #[arg(short, long)]
        /// Restrict search over text resources.
        language: Option<Language>,
        pattern: OsString,
    },
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

    let mut db = match args.xpac {
        Some(xpac) => Database::load_one(xpac),
        None => Database::load_all(),
    };

    match cmd {
        Action::Csv => commands::csv(&mut db),
        Action::Grep {
            pattern,
            ignore_case,
            fixed_strings,
            language,
        } => commands::grep(
            &mut db,
            pattern,
            ignore_case,
            fixed_strings,
            language,
        ),
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
            &mut db,
            path_or_item_name,
            difficulty.into(),
            enemy.into(),
            challenge,
            chest,
            affix,
            vendor,
            zero,
        ),
        Action::Item { name } => commands::item(&mut db, name),
        Action::Show { path } => commands::show(&mut db, path),
        Action::Tag { tag } => commands::tag(tag.to_string_lossy()),
    }
}

fn print_completions(shell: Shell) {
    let bin = std::env::args().next().unwrap_or_else(|| "gddb".to_string());
    let bin = std::path::Path::new(&bin)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gddb".to_string());
    let install_path = install_path();

    match shell {
        Shell::Bash => {
            println!("# Add this to your ~/.bashrc:");
            println!("export GRIM_DAWN_INSTALL_PATH={}", install_path.display());
            println!("source <(COMPLETE=bash {bin})");
        }
        Shell::Zsh => {
            println!("# Add this to your ~/.zshrc:");
            println!("export GRIM_DAWN_INSTALL_PATH={}", install_path.display());
            println!("source <(COMPLETE=zsh {bin})");
        }
        Shell::Fish => {
            println!("# Add this to your ~/.config/fish/config.fish:");
            println!("set -x GRIM_DAWN_INSTALL_PATH {}", install_path.display());
            println!("COMPLETE=fish {bin} | source");
        }
    }
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
