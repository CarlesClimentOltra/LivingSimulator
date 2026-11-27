//! Compiles `/data` into a canonical bincode blob embedded in the crate, and computes the
//! simulation version: a hash of the engine sources and the content. Two builds with the
//! same `SIM_VERSION` must produce bit-identical simulations.

use std::path::{Path, PathBuf};

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_sources(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let data_dir = manifest.join("../../data");
    let content_src = manifest.join("../livingsim-content/src");
    println!("cargo:rerun-if-changed={}", data_dir.display());
    println!(
        "cargo:rerun-if-changed={}",
        data_dir.join("names").display()
    );
    for f in livingsim_content::data_files(&data_dir) {
        println!("cargo:rerun-if-changed={}", f.display());
    }

    let content = match livingsim_content::load_dir(&data_dir) {
        Ok(c) => c,
        Err(e) => panic!("\n\nInvalid content in /data:\n{e}\n"),
    };
    let blob = bincode::serialize(&content).expect("serialize content");

    // Engine sources (core + content schema) are part of the version.
    let root = manifest.join("../..").canonicalize().unwrap();
    let mut sources = Vec::new();
    collect_sources(&manifest.join("src"), &mut sources);
    collect_sources(&content_src, &mut sources);
    let mut h = blake3::Hasher::new();
    h.update(b"livingsim-sim-version-v1\0");
    h.update(env!("CARGO_PKG_VERSION").as_bytes());
    for s in &sources {
        println!("cargo:rerun-if-changed={}", s.display());
        let abs = s.canonicalize().unwrap();
        let rel = abs
            .strip_prefix(&root)
            .expect("source inside workspace")
            .to_string_lossy()
            .replace('\\', "/");
        // Normalise line endings so Windows checkouts produce the same version.
        let text = std::fs::read_to_string(s).unwrap().replace("\r\n", "\n");
        h.update(rel.as_bytes());
        h.update(&(text.len() as u64).to_le_bytes());
        h.update(text.as_bytes());
    }
    h.update(&blob);
    let version = h.finalize();

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("content.bin"), &blob).unwrap();
    std::fs::write(out.join("sim_version.txt"), version.to_hex().as_str()).unwrap();
}
