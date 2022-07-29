mod mover;
mod transform_list;

use crate::transform_list::{TransformList, TransformListItem};
use clap::{Parser, Subcommand};
use colored::Colorize;
use log::debug;
use rayon::prelude::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::exit;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    dry_run: bool,

    /// Sets undo log path
    #[clap(short, long, default_value = "mrn_movelog.bak")]
    log: PathBuf,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Rename {
        /// Regular expression for filtering values and capturing substrings.
        regex: Regex,
        /// A pattern, written in Handlebars language. Regex capture groups can be accessed with {{_1}}, {{_2}}, etc.
        pattern: String,
        /// A path, which is used for collecting files, that will be renamed.
        path: PathBuf,

        /// Do not write undo log. Rename operation will be irreversible.
        #[clap(long)]
        no_log: bool,
    },
    /// Tries to reverse previous rename operation using undo log.
    Undo,
}

fn main() {
    env_logger::init();

    let args: Args = Args::parse();
    debug!("{:?}", args);

    let transform_list = match args.command {
        Commands::Rename {
            ref regex,
            ref pattern,
            ref path,
            ..
        } => TransformList::from_path(&regex, &pattern, &path),
        Commands::Undo => TransformList::from_undo(&args.log),
    }
    .unwrap();

    let no_log = match args.command {
        Commands::Rename { no_log, .. } => no_log,
        Commands::Undo => true, // do not write log on undo
    };

    // transform_list
    //     .iter()
    //     .for_each(|(orig, transformed)| println!("{orig:?} -> {transformed:?}"));

    eprintln!("{}", "== Checking for problems".bold());
    let problem_map = check_list(&transform_list);
    if problem_map.is_empty() {
        eprintln!("{}", "No problems detected".bold().green());
    } else {
        for TransformListItem { src, dest, .. } in transform_list.0.iter() {
            match problem_map.get(dest) {
                Some(Problem::Conflict) => println!(
                    "\"{}\" -> \"{}\" {}",
                    src.red(),
                    dest.red(),
                    "CONFLICT!".bright_red()
                ),
                Some(Problem::FileExists) => println!(
                    "\"{}\" -> \"{}\" {}",
                    src.red(),
                    dest.red(),
                    "FILE EXISTS!".bright_red()
                ),
                None => {}
            }
        }

        eprintln!(
            "{}",
            "Problems detected, cannot proceed with rename."
                .bold()
                .red()
        );
        exit(-1);
    }

    eprintln!("{}", "== Applying move".bold());
    mover::do_perform(&transform_list, args.dry_run, no_log, &args.log).unwrap();

    // println!("{:?}", ddd);
}

#[derive(Debug, Copy, Clone)]
enum Problem {
    Conflict,
    FileExists,
}

fn check_list(transform_list: &TransformList) -> HashMap<String, Problem> {
    let mut transform_hash = HashSet::with_capacity(transform_list.0.len());
    let mut problem_map = HashMap::new();

    debug!("Check for conflicts");
    for TransformListItem { dest, .. } in transform_list.0.iter() {
        if !transform_hash.insert(dest) {
            debug!("Got conflict on {dest}");
            let _ = problem_map.insert(dest.clone(), Problem::Conflict);
        }
    }

    let existing_files: HashMap<String, Problem> = transform_list
        .0
        .par_iter()
        .filter(|TransformListItem { dest, .. }| fs::metadata(dest).is_ok())
        .map(|TransformListItem { dest, .. }| (dest.clone(), Problem::FileExists))
        .collect();
    problem_map.extend(existing_files.iter().map(|(x, &y)| (x.clone(), y)));

    debug!("Problem map: {problem_map:?}");

    problem_map
}
