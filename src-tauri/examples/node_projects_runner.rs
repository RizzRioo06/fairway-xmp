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
    if std::env::args().nth(1).as_deref() == Some("--fixture-capabilities") {
        println!(
            "{}",
            serde_json::json!({"nodeInstallationMirrors":cfg!(all(debug_assertions,feature="node-install-fixtures"))})
        );
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--install-owner-fixture") {
        return installation_fixture();
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

// Stdio-only controlled host for abrupt-crash installation tests. No GUI.
fn installation_fixture() -> Result<(), String> {
    use fairway_core::node_project_creation as creation;
    use std::sync::Arc;
    let root = PathBuf::from(
        std::env::args_os()
            .nth(2)
            .ok_or("Missing fixture directory")?,
    );
    if !root.is_absolute() || !root.is_dir() {
        return Err("Expected existing absolute fixture directory".into());
    }
    let layout = Layout::at(root.join("home"), root.join("data"), root.join("tools"));
    let state = Arc::new(creation::NodeProjectCreation::new(Some(layout.clone())));
    let nodes = Arc::new(NodeProjects::new(Some(layout)));
    for line in io::stdin().lock().lines() {
        let value: serde_json::Value =
            serde_json::from_str(&line.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let result: creation::Result<serde_json::Value> = (|| {
            Ok(match value["command"].as_str() {
                Some("catalog") => serde_json::to_value(state.catalog()).unwrap(),
                Some("plan") => serde_json::to_value(state.plan(
                    serde_json::from_value(value["request"].clone()).map_err(|e| {
                        creation::Error {
                            code: creation::ErrorCode::InvalidRequest,
                            message: e.to_string(),
                        }
                    })?,
                )?)
                .unwrap(),
                Some("start") => serde_json::to_value(state.start_installation(
                    nodes.clone(),
                    serde_json::from_value(value["request"].clone()).map_err(|e| {
                        creation::Error {
                            code: creation::ErrorCode::InvalidRequest,
                            message: e.to_string(),
                        }
                    })?,
                )?)
                .unwrap(),
                _ => {
                    return Err(creation::Error {
                        code: creation::ErrorCode::InvalidRequest,
                        message: "Unknown fixture command".into(),
                    })
                }
            })
        })();
        println!(
            "{}",
            match result {
                Ok(value) => serde_json::json!({"value":value}),
                Err(error) => serde_json::json!({"error":error}),
            }
        );
        io::stdout().flush().map_err(|e| e.to_string())?;
    }
    state.cancel_installations_for_shutdown();
    state.stop_installations().map_err(|e| e.message)
}
