use colored::Colorize;
use handlebars::{handlebars_helper, Handlebars};
use heck::ToTitleCase;
use log::debug;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{fs, io};
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Move,
    Copy,
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
pub struct TransformListItem {
    pub src: String,
    pub dest: String,
    pub action: Action,
}

impl TransformListItem {
    fn reverse(self) -> Self {
        Self {
            src: self.dest,
            dest: self.src,
            action: Action::Move, // Ensures, that original file is created, then deletes copied file
        }
    }
}

pub struct TransformList(pub Vec<TransformListItem>);

const TEMPLATE_NAME: &str = "name_template";

handlebars_helper!(uppercase: |x: String| x.to_uppercase());
handlebars_helper!(lowercase: |x: String| x.to_lowercase());
handlebars_helper!(titlecase: |x: String| x.to_title_case());

impl TransformList {
    pub fn from_path(regex: &Regex, pattern: &str, path: &Path) -> io::Result<Self> {
        eprintln!("{}", "== Creating file list".bold());

        let file_list = collect_files(&path, &regex);
        eprintln!("Got {} file(s) in {path:?}", file_list.len());

        Self::from_files(regex, pattern, &file_list)
    }

    pub fn from_files(regex: &Regex, pattern: &str, files: &[String]) -> io::Result<Self> {
        eprintln!("{}", "== Applying transformations".bold());
        let handlebars = {
            let mut handlebars_inner = Handlebars::new();
            handlebars_inner
                .register_template_string(TEMPLATE_NAME, &pattern)
                .unwrap();
            handlebars_inner.register_helper("uppercase", Box::new(uppercase));
            handlebars_inner.register_helper("lowercase", Box::new(lowercase));
            handlebars_inner.register_helper("titlecase", Box::new(titlecase));
            handlebars_inner
        };

        let file_list = files
            .par_iter()
            .enumerate()
            .map(|(number, name)| {
                let orig_name = name.clone();
                let transformed = transform_name(&orig_name, &regex, &handlebars, number);
                TransformListItem {
                    src: orig_name,
                    dest: transformed,
                    action: Action::Move,
                }
            })
            .collect();

        Ok(Self(file_list))
    }

    pub fn from_undo(undo_log: &Path) -> io::Result<Self> {
        eprintln!("{}", "== Collecting files from undo log".bold());

        let mut transform_list = vec![];

        let mut undo_log_f = io::BufReader::new(fs::File::open(undo_log)?);
        loop {
            match jsonl::read(&mut undo_log_f) {
                Ok(val) => transform_list.push(TransformListItem::reverse(val)),
                Err(jsonl::ReadError::Eof) => return Ok(Self(transform_list)),
                Err(jsonl::ReadError::Io(e)) => return Err(e),
                Err(jsonl::ReadError::Deserialize(e)) => {
                    return Err(io::Error::new(ErrorKind::Other, e.to_string()))
                }
            }
        }
    }
}

fn collect_files(root_dir: &Path, regex_filter: &Regex) -> Vec<String> {
    // Walk root path synchronously
    let file_list: walkdir::Result<Vec<PathBuf>> = WalkDir::new(root_dir)
        .sort_by_file_name()
        .into_iter()
        .map(|c| c.map(|a| a.path().to_path_buf()))
        .collect();

    let file_list = file_list.unwrap();

    // Apply check file type and apply regex
    file_list
        .par_iter()
        .filter(|d| d.is_file())
        .map(|d| d.to_str().unwrap())
        .filter(|d| regex_filter.is_match(d))
        .map(|x| x.to_owned())
        .collect()
}

fn transform_name(src: &str, r: &Regex, handlebars: &Handlebars, number: usize) -> String {
    let mut context: HashMap<String, Value> = HashMap::new();

    for cap in r.captures_iter(src) {
        for sub in cap.iter().enumerate() {
            context.insert(format!("_{}", sub.0), sub.1.unwrap().as_str().into());
        }
    }

    context.insert("__self".into(), src.into());
    context.insert("__n".into(), number.into());

    debug!("Render context: {context:?}");
    let rendered = handlebars.render(TEMPLATE_NAME, &context).unwrap();

    debug!("Rendered: {rendered:?}");

    rendered
}
