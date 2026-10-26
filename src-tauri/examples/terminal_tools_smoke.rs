//! Headless check of this Mac user's actual embedded-terminal tool discovery.
//! Prints only selected command paths/Node version, never the captured exports.
#[cfg(target_os = "macos")]
fn main() -> Result<(), String> {
    use fairway_core::{
        config::Layout,
        terminal::{StartRequest, Terminals},
    };
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    let layout = Layout::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"))?;
    let terminals = Terminals::default();
    let result = (|| {
        terminals.start(
            &layout,
            StartRequest {
                id: "tools-check".into(),
                workspace: "tools-check".into(),
                slot: 0,
                cols: 160,
                rows: 40,
                folder: None,
                remember_history: false,
            },
        )?;
        terminals.write("tools-check", "for tool in node npm npx git python3 brew cargo php composer fnm; do printf '__tool__%s=' \"$tool\"; whence -p -- \"$tool\" || printf 'unavailable\\n'; done; printf '__node_version__'; node --version; printf '%s%s\\n' FAIRWAY_ TOOLS_DONE\r")?;
        let mut bytes = Vec::new();
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            let output =
                serde_json::to_value(terminals.read("tools-check")?).map_err(|e| e.to_string())?;
            let data: Vec<u8> =
                serde_json::from_value(output["data"].clone()).map_err(|e| e.to_string())?;
            bytes.extend(data);
            if bytes.len() > 256 * 1024 {
                return Err("Unexpected terminal output size.".into());
            }
            let text = String::from_utf8_lossy(&bytes);
            if text.contains("FAIRWAY_TOOLS_DONE") {
                let mut node = false;
                let mut own_php = false;
                let mut own_composer = false;
                for line in text.lines().map(|line| line.trim_matches(['\r', '\n'])) {
                    if let Some(entry) = line.strip_prefix("__tool__") {
                        if let Some((name, path)) = entry.split_once('=') {
                            if path.starts_with('/') || path == "unavailable" {
                                println!("{name}: {path}");
                                node |= name == "node" && path.starts_with('/');
                                own_php |= name == "php" && path.contains("terminal-bin/php");
                                own_composer |=
                                    name == "composer" && path.contains("terminal-bin/composer");
                            }
                        }
                    }
                    if let Some(version) = line.strip_prefix("__node_version__v") {
                        if version.chars().all(|c| c.is_ascii_digit() || c == '.') {
                            println!("Node: v{version}");
                        }
                    }
                }
                if !node || !own_php || !own_composer {
                    return Err(
                        "Node discovery or Fairway PHP/Composer priority did not pass.".into(),
                    );
                }
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err("Terminal tool check timed out.".into())
    })();
    terminals.close_all();
    result
}
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Run this current-user diagnostic on macOS.");
}
