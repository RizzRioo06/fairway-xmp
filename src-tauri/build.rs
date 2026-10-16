use std::{env, path::PathBuf, process::Command};

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("..");
    let git = |args: &[&str]| -> Option<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    println!("cargo:rerun-if-env-changed=FAIRWAY_BUILD_COMMIT");
    for path in [
        "src",
        "examples",
        "Cargo.toml",
        "tauri.conf.json",
        "../src",
        "../package.json",
        "../runtime-manifest.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(&["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={}", root.join(path).display());
        }
    }
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            println!("cargo:rerun-if-changed={}", root.join(path).display());
        }
    }
    let commit = env::var("FAIRWAY_BUILD_COMMIT")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .filter(|value| value.len() == 40 && value.bytes().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FAIRWAY_BUILD_COMMIT={commit}");
    let dirty = git(&["status", "--porcelain", "--untracked-files=normal"])
        .is_some_and(|value| !value.is_empty());
    println!("cargo:rustc-env=FAIRWAY_BUILD_DIRTY={dirty}");
    #[cfg(feature = "desktop")]
    {
        tauri_build::build();
        if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
            && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        {
            // Tauri embeds its resource in application binaries, but Cargo
            // examples need their own Common Controls v6 activation manifest.
            // Without it Windows cannot load TaskDialogIndirect in the probe.
            let manifest = root.join("packaging/windows/probe.manifest");
            println!("cargo:rerun-if-changed={}", manifest.display());
            println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
            println!(
                "cargo:rustc-link-arg-examples=/MANIFESTINPUT:{}",
                manifest.display()
            );
        }
    }
}
