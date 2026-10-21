//! Check first-run and legacy MySQL recovery against an isolated bundled runtime.
#[path = "support/mysql_recovery.rs"]
mod mysql_recovery;

use std::{fs, path::PathBuf};

fn main() -> Result<(), String> {
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    let root = directory.join(
        std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .ok_or("Provide a fresh test directory.")?,
    );
    if root.exists()
        && fs::read_dir(&root)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err("Use a fresh isolated test directory; existing files were kept.".into());
    }
    let runtime = directory.join(
        std::env::args()
            .nth(2)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime")),
    );
    mysql_recovery::run(&root, &runtime)
}
