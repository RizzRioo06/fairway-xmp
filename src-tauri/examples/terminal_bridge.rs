//! Test-only stdio bridge: exercise the React terminal against the real Rust PTY
//! and packaged PHP/Composer, without exposing a server or touching user data.
use fairway_core::{config::Layout, manager::Manager, terminal::Terminals};
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    let root = PathBuf::from(
        args.get(1)
            .ok_or("Pass an empty isolated test directory and runtime path.")?,
    );
    let runtime = PathBuf::from(args.get(2).ok_or("Pass the packaged runtime path.")?);
    if !root.is_absolute() || !runtime.is_absolute() || root.exists() {
        return Err(
            "Use a new absolute isolated test directory and an absolute runtime path.".into(),
        );
    }
    let layout = Layout::at(
        root.join("Student May's home 日本"),
        root.join("data"),
        runtime,
    );
    let mut manager = Manager::new(layout).map_err(io::Error::other)?;
    let terminals = Terminals::default();
    for line in io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        let args = &request["args"];
        let id = args["id"].as_str().unwrap_or_default();
        let result: Result<Value, String> = (|| {
            Ok(match request["command"].as_str().unwrap_or_default() {
                "snapshot" => serde_json::to_value(manager.snapshot()).unwrap(),
                "plugin:event|listen" => json!(1),
                "plugin:event|unlisten" => Value::Null,
                "terminal_start" => serde_json::to_value(terminals.start(
                    &manager.layout,
                    id.into(),
                    args["cols"].as_u64().unwrap_or(80) as u16,
                    args["rows"].as_u64().unwrap_or(24) as u16,
                )?)
                .unwrap(),
                "terminal_read" => serde_json::to_value(terminals.read(id)?).unwrap(),
                "terminal_write" => {
                    terminals.write(id, args["data"].as_str().unwrap_or_default())?;
                    Value::Null
                }
                "terminal_resize" => {
                    terminals.resize(
                        id,
                        args["cols"].as_u64().unwrap_or(80) as u16,
                        args["rows"].as_u64().unwrap_or(24) as u16,
                    )?;
                    Value::Null
                }
                "terminal_close" => {
                    terminals.close(id);
                    Value::Null
                }
                _ => return Err("Unsupported test bridge command.".into()),
            })
        })();
        let response = match result {
            Ok(value) => json!({"requestId": request["requestId"], "value": value}),
            Err(error) => json!({"requestId": request["requestId"], "error": error}),
        };
        println!("{response}");
        io::stdout().flush()?;
    }
    terminals.close_all();
    Ok(())
}
