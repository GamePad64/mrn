use crate::transform_list::{Action, TransformListItem};
use crate::TransformList;
use colored::Colorize;
use log::{debug, trace};
use std::fs::File;
use std::path::Path;
use std::{fs, io};

fn perform_op(item: &TransformListItem) -> io::Result<()> {
    trace!("perform_op({:?})", item);

    if let Some(parent) = Path::new(&item.dest).parent() {
        fs::create_dir_all(parent)?;
    }

    if item.action == Action::Move {
        debug!("Trying to move using fs::rename");
        match fs::rename(&item.src, &item.dest) {
            Ok(_) => return Ok(()),
            Err(e) => {
                debug!("Couldn't move using fs::rename: {e:?}");
            }
        }
    }

    debug!("Trying to move/copy using block copy");
    fs::copy(&item.src, &item.dest)?;

    if item.action == Action::Move {
        debug!("Deleting file");
        fs::remove_file(&item.src)?;
    }

    Ok(())
}

fn do_perform_one(
    item: &TransformListItem,
    dry_run: bool,
    undo_log: Option<&File>,
) -> io::Result<()> {
    if !dry_run {
        perform_op(item)?;

        if let Some(undo_log) = undo_log {
            jsonl::write(undo_log, item).unwrap();
        }
    }

    Ok(())
}

pub fn do_perform(
    files: &TransformList,
    dry_run: bool,
    no_log: bool,
    movelog: &Path,
) -> io::Result<()> {
    let movelog_f = match (no_log, dry_run) {
        (false, false) => {
            eprintln!("Writing undo log to {movelog:?}");
            Some(
                fs::OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(movelog)?,
            )
        }
        (true, _) => {
            eprintln!("{}", "Not writing undo log because of --no-log".yellow());
            None
        }
        (_, true) => {
            eprintln!("{}", "Not writing undo log because of --dry-run".yellow());
            None
        }
    };

    for item in files.0.iter() {
        do_perform_one(item, dry_run, movelog_f.as_ref())?;
    }
    Ok(())
}
