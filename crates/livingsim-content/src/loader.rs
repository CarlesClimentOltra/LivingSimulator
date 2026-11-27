//! Loading `/data` from disk (feature `loader`).

use crate::raw::*;
use crate::{compile, Content, ContentError};
use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};

fn read<T: DeserializeOwned>(path: &Path, problems: &mut Vec<String>) -> Option<T> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            problems.push(format!("{}: {e}", path.display()));
            return None;
        }
    };
    let opts =
        ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    match opts.from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            problems.push(format!("{}: {e}", path.display()));
            None
        }
    }
}

/// Every file read by [`load_dir`] (useful for `cargo:rerun-if-changed`).
pub fn data_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for sub in [dir.to_path_buf(), dir.join("names")] {
        if let Ok(rd) = std::fs::read_dir(&sub) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "ron") {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    files
}

/// Loads, validates and compiles every RON file in `dir`.
pub fn load_dir(dir: &Path) -> Result<Content, ContentError> {
    let mut p = Vec::new();
    let mut raw = RawContent {
        needs: read(&dir.join("needs.ron"), &mut p).unwrap_or_default(),
        activities: read(&dir.join("activities.ron"), &mut p).unwrap_or_default(),
        schedules: read(&dir.join("schedules.ron"), &mut p).unwrap_or_default(),
        professions: read(&dir.join("professions.ron"), &mut p).unwrap_or_default(),
        buildings: read(&dir.join("buildings.ron"), &mut p).unwrap_or_default(),
        name_styles: vec![],
        names_common: read(&dir.join("names/common.ron"), &mut p),
        text: read(&dir.join("journal.ron"), &mut p),
        economy: read(&dir.join("economy.ron"), &mut p),
        world: read(&dir.join("world.ron"), &mut p),
        goods: read(&dir.join("goods.ron"), &mut p).unwrap_or_default(),
        crimes: read(&dir.join("crimes.ron"), &mut p).unwrap_or_default(),
        technologies: read(&dir.join("technologies.ron"), &mut p).unwrap_or_default(),
        demography: read(&dir.join("demography.ron"), &mut p),
    };
    // Every names/style_*.ron is a name style, in file-name order.
    let mut style_files: Vec<_> = data_files(dir)
        .into_iter()
        .filter(|f| {
            f.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("style_"))
        })
        .collect();
    style_files.sort();
    for f in style_files {
        if let Some(s) = read::<NameStyle>(&f, &mut p) {
            raw.name_styles.push(s);
        }
    }
    if !p.is_empty() {
        return Err(ContentError { problems: p });
    }
    compile(&raw)
}
