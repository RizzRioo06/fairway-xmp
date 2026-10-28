//! Test transport for controlled Node fixtures; production uses the app binary.
use fairway_core::{
    config::Layout,
    node_projects::{self, NodeProjects},
};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

fn main() -> Result<(), String> {
    if node_projects::runner_entrypoint().map_err(|e| e.message)? {
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() != Some("--owner-fixture") {
        return Err("Pass --owner-fixture and an isolated test directory.".into());
    }
    let root = PathBuf::from(
        std::env::args_os()
            .nth(2)
            .ok_or("Missing isolated directory")?,
    );
    if !root.is_absolute() || !root.is_dir() {
        return Err("Use an existing absolute fixture directory.".into());
    }
    let projects = NodeProjects::new(Some(Layout::at(
        root.join("home"),
        root.join("data"),
        root.join("runtime"),
    )));
    for line in io::stdin().lock().lines() {
        let value: serde_json::Value =
            serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let response = match value["command"].as_str() {
            Some("start") => projects
                .start(serde_json::from_value(value["request"].clone()).map_err(|e| e.to_string())?)
                .map(|run| serde_json::to_value(run).unwrap()),
            Some("runs") => Ok(serde_json::to_value(projects.runs()).unwrap()),
            Some("logs") => projects
                .logs(serde_json::from_value(value["request"].clone()).map_err(|e| e.to_string())?)
                .map(|logs| serde_json::to_value(logs).unwrap()),
            Some("stop") => projects
                .stop(serde_json::from_value(value["request"].clone()).map_err(|e| e.to_string())?)
                .map(|run| serde_json::to_value(run).unwrap()),
            _ => Err(node_projects::Error {
                code: node_projects::ErrorCode::InvalidProject,
                message: "Unknown fixture command".into(),
            }),
        };
        println!(
            "{}",
            match response {
                Ok(value) => serde_json::json!({"value":value}),
                Err(error) => serde_json::json!({"error":error}),
            }
        );
        io::stdout().flush().map_err(|e| e.to_string())?;
    }
    projects.stop_all_runs().map_err(|e| e.message)
}
