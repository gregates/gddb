## Overview

gddb is a command-line interface into the game data files for Grim Dawn, an action RPG created by
[Crate Entertainment](https://www.crateentertainment.com/). It can be used to explore the game
files, and has several utilities for **resolving** the game files into more human-readable
information. The motivating example is generating complete loot tables for monster infrequents
(MIs) (see the `loot` command).

gddb is a work in progress and should be considered alpha software. It is built with rust and has
only been tested on Ubuntu running on the Windows Subsystem for Linux (WSL) against game verison
1.2.1.6.

The author is not affilitated with Crate and cannot promise that the output of this program is
correct. I have had to make several assumptions about how to interpret the game files; any one of
them could be wrong. Compatibilty with past and future versions of the game is not guaranteed.

## Installation

[Install rust](https://www.rust-lang.org/tools/install)

```
git clone https://github.com/gregates/gddb.git
cd gddb
cargo install --path .
```

Database files are not included. You must have Grim Dawn installed on your system for the program
to work. You can specify the path to the game files by
setting the `GRIM_DAWN_INSTALL_PATH` env variable.

On WSL, I recommend copying the data to a location in your linux filesystem, or all
commands will run more slowly.

## Commands

### Show
Browse database files, with the files from all xpacs merged into a single file tree.
```
$ gddb show records/items/loottables/gearaccessories | head
lt_medal.dbr
lt_medal_a200.dbr
lt_medal_b010_balthazar_01.dbr
lt_medal_b011_moneybags_01.dbr
lt_medal_b012_halion_01.dbr
lt_medal_b013_bloodfeast_01.dbr
lt_medal_b014_gutworm_01.dbr
lt_medal_b015_nacrathan_01.dbr
lt_medal_b016_harvoul_01.dbr
lt_medal_b111_wendigoancient.dbr

$ gddb show records/items/loottables/gearaccessories/lt_medal_b010_balthazar_01.dbr
id: records/items/loottables/gearaccessories/lt_medal_b010_balthazar_01.dbr
kind: LevelTable
=======================
  levels=1,15
  records=records/items/loottables/geartorso/tdyn_torsolight_a01.dbr,records/items/loottables/gearaccessories/tdyn_medal_b10_balthazar.dbr
  Class=LevelTable
  templateName=database/templates/leveltable.tpl
```

#### Tab completion

The show command supports tab auto-complete of database file paths! This requires some setup in your
shell. See `gddb --completions [SHELL]` for instructions.

### Grep
Searches all database files and text localization resources whose contents match the provided regex.
Prints expac (if any), record/file path, line number, and line.
```
$ gddb grep -l en -i "gun2hc\d03" 
records/items/gearweapons/guns2h/c007_gun2h.dbr:161:itemNameTag=tagWeaponGun2hC003
records/items/gearweapons/guns2h/c036_gun2h.dbr:162:itemNameTag=tagWeaponGun2hC003
gdx1:records/items/upgraded/gearweapons/guns2h/c036_gun2h.dbr:166:itemNameTag=tagWeaponGun2hC003
gdx1:records/items/gearweapons/guns2h/c103_gun2h.dbr:166:itemNameTag=tagGDX1WeaponGun2hC103
gdx1:records/items/gearweapons/guns2h/c007_gun2h.dbr:163:itemNameTag=tagWeaponGun2hC003
gdx1:records/items/gearweapons/guns2h/c036_gun2h.dbr:164:itemNameTag=tagWeaponGun2hC003
gdx1:records/items/gearweapons/guns2h/c109_gun2h.dbr:166:itemNameTag=tagGDX1WeaponGun2hC103
gdx2:records/items/gearweapons/guns2h/c109_gun2h.dbr:168:itemNameTag=tagGDX1WeaponGun2hC103
gdx2:records/items/gearweapons/guns2h/c007_gun2h.dbr:165:itemNameTag=tagWeaponGun2hC003
gdx2:records/items/gearweapons/guns2h/c103_gun2h.dbr:168:itemNameTag=tagGDX1WeaponGun2hC103
gdx2:records/items/gearweapons/guns2h/c036_gun2h.dbr:166:itemNameTag=tagWeaponGun2hC003
gdx2:records/items/upgraded/gearweapons/guns2h/c036_gun2h.dbr:168:itemNameTag=tagWeaponGun2hC003
gdx1:resources/Text_EN.arc/tagsgdx1_items.txt:118:tagGDX1WeaponGun2hC103=Burning Purifier
resources/Text_EN.arc/tags_items.txt:306:tagWeaponGun2hC003=Hellmaw Shotgun
```

### Tag
Resolve a text tag, using the specified localization (default: English).
```
$ gddb tag tagGDX2FocusB206
Abaddoth's Sermons

$ gddb --language zh tag tagGDX2FocusB206
阿巴多斯的布道
```

### Item
Find the database files referencing an item by substring match on the item's name.

```
$ gddb item Moosi
Multiple tags found, please disambiguate:
  Moosilauke's Pauldrons
  Moosilauke's Shoulderguards

$ gddb item "Moosilauke's Shoulderguards"
Moosilauke's Shoulderguards is referenced in the following database records:
  records/items/gearshoulders/b018c_shoulder.dbr
  records/items/gearshoulders/b018a_shoulder.dbr
  records/items/gearshoulders/b018e_shoulder.dbr
  records/items/gearshoulders/b018d_shoulder.dbr
  records/items/gearshoulders/b018b_shoulder.dbr
```

### Loot
Resolve the loot table for an item. Accepts either a full path to a loot table file, or a (partial)
item name. If given the latter, will use the highest level version of the item. Can specify
parameters that modify the affix chances, like difficulty (default: Ultimate), enemy class
(default: Boss), and challenge layer (default: none).

```
$ gddb loot "Wendigo Gaze" | head -n 9
3.16164605%     Wraithbound             of Mending
1.31735252%     Wraithbound             of Attack
1.31735252%     Wraithbound             of Protection
1.23802816%     Aggressive              of the Wild
1.23802816%     Aggressive              of the Untamed
1.23802816%     Aggressive              of Caged Souls
1.23802816%     Stalwart                of the Wild
1.23802816%     Stalwart                of the Untamed
1.23802816%     Stalwart                of Caged Souls

$ gddb loot --difficulty normal --enemy champion "Wendigo Gaze" | head -n 9
1.71753967%     Aggressive              of Mending
1.71753967%     Stalwart                of Mending
1.71753967%     Shrewd                  of Mending
1.71753967%     Mighty                  of Mending
1.71753967%     Mystic                  of Mending
1.71753967%     Vigorous                of Mending
1.41697022%     Prismatic               of Mending
1.41697022%     Impervious              of Mending
0.71564153%     Aggressive              of Attack
```

### CSV
Outputs a csv file containing the fully resolved loot table for **every** MI in the game. Useful
for exploring these loot tables in a sufficiently powerful spreadsheet program, such as [Row
Zero](https://rowzero.io/home).

## FAQ

### Why not just use ArchiveTool.exe and the modding tools that come with the game?

When I first started exploring the game database, I did use ArchiveTool.exe to extract the game
files to my filesystem, and explored them that way. But this is somewhat annoying to do and takes
up disk space! And it slightly offends my programmer's sensibilities. The arz and arc file formats
aren't that complicated, and we can just read the files in-place without having to extract them
first.

Plus, this utility is composable with other programs, and can automate tasks (like resolving loot
tables) that are a massive pain to do by manually reading the db files.
