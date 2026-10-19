//! Writes `tests/mermaid/compat/` in a checkout: Pele's `parse` output for the
//! sources in Pele's ported specs, by `tests/mermaid/pele/render.mjs compat`.
//!
//! Needs `curl`, `tar`, Node and npm. Without one, the build goes on with a
//! warning and the compat tests fail on the missing files. The published
//! crate has no `tests/`, so this does nothing there.

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::Path,
    process::Command,
};

/// The Pele commit the specs are read from, matching the `pele` version
/// pinned in `tests/mermaid/pele/package.json`.
const PELE: &str = "44da152";

/// Written beside the compat files, naming the commit and generator that
/// made them.
const STAMP: &str = ".stamp";

fn main() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mermaid");
    let scripts = tests.join("pele");
    if !scripts.is_dir() {
        return;
    }
    let generator = scripts.join("render.mjs");
    let lock = scripts.join("package-lock.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", generator.display());
    println!("cargo:rerun-if-changed={}", lock.display());

    let out = tests.join("compat");
    let stamp = stamp(&generator, &lock);
    if fs::read_to_string(out.join(STAMP)).is_ok_and(|found| found == stamp) {
        return;
    }
    if let Err(error) = generate(&scripts, &out) {
        println!("cargo:warning=tests/mermaid/compat not written: {error}");
        return;
    }
    fs::write(out.join(STAMP), stamp).expect("writing the compat stamp");
}

fn stamp(generator: &Path, lock: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    for file in [generator, lock] {
        fs::read(file)
            .expect("reading a compat input")
            .hash(&mut hasher);
    }
    format!("{PELE} {:016x}", hasher.finish())
}

fn generate(scripts: &Path, out: &Path) -> Result<(), String> {
    if !scripts.join("node_modules/pele").is_dir() {
        let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
        run(
            Command::new(npm)
                .args(["ci", "--no-audit", "--no-fund"])
                .current_dir(scripts),
            npm,
        )?;
    }

    let work = Path::new(&std::env::var("OUT_DIR").unwrap()).join("pele");
    let checkout = work.join(PELE);
    if !checkout.join("tests/compat").is_dir() {
        fs::create_dir_all(&checkout).map_err(|e| e.to_string())?;
        let archive = work.join(format!("{PELE}.tar.gz"));
        run(
            Command::new("curl")
                .args(["-sSL", "--fail", "-o"])
                .arg(&archive)
                .arg(format!(
                    "https://codeload.github.com/obsidianmd/pele/tar.gz/{PELE}"
                )),
            "curl",
        )?;
        run(
            Command::new("tar")
                .arg("-xzf")
                .arg(&archive)
                .arg("-C")
                .arg(&checkout)
                .arg("--strip-components=1"),
            "tar",
        )?;
    }

    run(
        Command::new("node")
            .arg("render.mjs")
            .arg("compat")
            .arg(&checkout)
            .arg(out)
            .current_dir(scripts),
        "node",
    )
}

fn run(command: &mut Command, name: &str) -> Result<(), String> {
    match command.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{name} exited with {status}")),
        Err(error) => Err(format!("{name}: {error}")),
    }
}
