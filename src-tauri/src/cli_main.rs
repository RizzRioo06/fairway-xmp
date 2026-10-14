#[cfg(unix)]
fn main() {
    if let Err(error) = fairway_core::cli::run() {
        eprintln!("Fairway XMP: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(unix))]
fn main() {
    eprintln!("The terminal edition requires Linux.");
    std::process::exit(1);
}
