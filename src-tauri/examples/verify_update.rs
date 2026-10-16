//! Offline verification used before publishing the exact tested update files.
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err("Usage: verify_update PUBLIC_KEY ARTIFACT SIGNATURE VERSION".into());
    }
    let key = String::from_utf8(STANDARD.decode(fs::read_to_string(&args[0])?.trim())?)?;
    let signature = String::from_utf8(STANDARD.decode(fs::read_to_string(&args[2])?.trim())?)?;
    let key = PublicKey::decode(&key)?;
    let signature = Signature::decode(&signature)?;
    key.verify(&fs::read(&args[1])?, &signature, true)?;
    let version = signature
        .trusted_comment()
        .split('\t')
        .find_map(|s| s.strip_prefix("version:"));
    if version != Some(args[3].as_str()) {
        return Err("The authenticated version does not match the release version".into());
    }
    println!("Verified artifact signature and release version.");
    Ok(())
}
