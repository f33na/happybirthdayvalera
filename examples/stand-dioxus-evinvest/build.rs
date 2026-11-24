//! Writes the kit's class inventory and token sheet next to `tailwind.css`, as the `ev_lib` README asks:
//! Tailwind can neither scan nor `@import` a crate from the registry. Both files are ignored by git, so they
//! always match the `ev_lib_classes` version in `Cargo.lock`.

use std::path::Path;
use std::{fs, io};

/// Writes only on a change, so `dx serve` does not see a new file on every build and rebuild in a loop.
fn write_if_changed(path: &Path, contents: &str) -> io::Result<()> {
    if fs::read_to_string(path).is_ok_and(|old| old == contents) {
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, contents)
}

fn main() -> io::Result<()> {
    println!("cargo::rerun-if-changed=build.rs");
    // A deleted output makes Cargo run the script again; without these lines it would stay missing.
    println!("cargo::rerun-if-changed=uikit-classes.txt");
    println!("cargo::rerun-if-changed=assets/tokens.css");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    write_if_changed(&root.join("uikit-classes.txt"), ev_lib_classes::CLASS_INVENTORY)?;
    write_if_changed(&root.join("assets/tokens.css"), ev_lib_classes::TOKENS_CSS)?;
    // `asset!` needs the Tailwind output to exist. On a fresh clone, `dx` runs Tailwind before this script
    // has written tokens.css, so a plain `cargo check` comes first and finds this empty stand-in.
    let tailwind = root.join("assets/tailwind.css");
    if !tailwind.exists() {
        fs::write(tailwind, "")?;
    }
    Ok(())
}
