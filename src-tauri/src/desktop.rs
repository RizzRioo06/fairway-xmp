use crate::{
    config::{Layout, Ports, Result},
    integration,
    manager::{Manager, Snapshot},
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager as _, State};
use tauri_plugin_updater::UpdaterExt;

type Shared = Arc<Mutex<Manager>>;

async fn access<T: Send + 'static>(
    state: Shared,
    f: impl FnOnce(&mut Manager) -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut manager = state
            .lock()
            .map_err(|_| "The service manager is unavailable. Restart Fairway.")?;
        f(&mut manager)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn snapshot(state: State<'_, Shared>) -> Result<Snapshot> {
    access(state.inner().clone(), |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn service_action(
    state: State<'_, Shared>,
    action: String,
    service: Option<String>,
) -> Result<Snapshot> {
    access(state.inner().clone(), move |m| {
        match action.as_str() {
            "start_all" => m.start_all()?,
            "stop_all" => m.stop_all()?,
            "start" => m.start(service.as_deref().ok_or("Choose a service.")?)?,
            "stop" => m.stop(service.as_deref().ok_or("Choose a service.")?)?,
            "restart" => {
                let id = service.as_deref().ok_or("Choose a service.")?;
                let restore_web = id == "php"
                    && m.snapshot()
                        .services
                        .iter()
                        .any(|s| s.id == "nginx" && s.running);
                m.stop(id)?;
                m.start(id)?;
                if restore_web {
                    m.start("nginx")?;
                }
            }
            _ => return Err("Unknown action.".into()),
        }
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn save_ports(state: State<'_, Shared>, ports: Ports) -> Result<Snapshot> {
    access(state.inner().clone(), move |m| {
        m.save_ports(ports)?;
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn open_target(state: State<'_, Shared>, target: String) -> Result<()> {
    access(state.inner().clone(), move |m| {
        integration::open(&m.layout, &target, m.ports.website, m.ports.phpmyadmin)
    })
    .await
}

#[tauri::command]
async fn read_log(state: State<'_, Shared>, service: String) -> Result<String> {
    access(state.inner().clone(), move |m| m.log(&service)).await
}

#[tauri::command]
async fn cli_integration(state: State<'_, Shared>, enabled: bool) -> Result<Snapshot> {
    access(state.inner().clone(), move |m| {
        if enabled {
            integration::install(&m.layout)?;
        } else {
            integration::remove(&m.layout)?;
        }
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn backup_database(state: State<'_, Shared>) -> Result<String> {
    access(state.inner().clone(), |m| m.backup()).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    configured: bool,
    version: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo> {
    // The release workflow injects a real update endpoint and public key.
    if !app.config().plugins.0.contains_key("updater") {
        return Ok(UpdateInfo {
            configured: false,
            version: None,
            notes: None,
        });
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(UpdateInfo {
        configured: true,
        version: update.as_ref().map(|u| u.version.clone()),
        notes: update.and_then(|u| u.body),
    })
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<()> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No update is available.")?;
    // Stop and make a logical backup before replacing any bundled runtime.
    access(state.inner().clone(), |m| {
        if m.layout.data.join("database-ready").exists() {
            m.start("mysql")?;
            m.backup()?;
        }
        m.stop_all()
    })
    .await?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

pub fn runtime_location(exe: &std::path::Path) -> Result<PathBuf> {
    let directory = exe.parent().ok_or("Cannot locate the application.")?;
    #[cfg(target_os = "macos")]
    {
        Ok(directory.join("../Resources/runtime"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(directory.join("runtime"))
    }
}

// Installer hooks use the same tested integration code as the panel.
pub fn maintenance(args: &[String]) -> Result<bool> {
    let Some(action) = args.get(1) else {
        return Ok(false);
    };
    if ![
        "--install-cli",
        "--remove-cli",
        "--remove-data",
        "--remove-coursework",
        "--prepare-uninstall",
    ]
    .contains(&action.as_str())
    {
        return Ok(false);
    }
    let runtime = runtime_location(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    let layout = Layout::new(runtime)?;
    if [
        "--prepare-uninstall",
        "--remove-data",
        "--remove-coursework",
    ]
    .contains(&action.as_str())
    {
        for name in ["app.pid", "mysql.pid", "nginx.pid", "php.pid"] {
            let path = layout.data.join(name);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let pid: u32 = text.trim().parse().map_err(|_| {
                    "Cannot verify a service PID; stop services before uninstalling."
                })?;
                if crate::process::pid_alive(pid) {
                    return Err(
                        "Open Fairway, stop all services, and quit before uninstalling.".into(),
                    );
                }
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
    }
    match action.as_str() {
        "--install-cli" => integration::install(&layout)?,
        "--remove-cli" => integration::remove(&layout)?,
        "--remove-data" => {
            integration::remove(&layout)?;
            // Refuse cleanup while an owned server may still be using the data directory.
            if layout.data.join("mysql.pid").exists() || layout.data.join("nginx.pid").exists() {
                return Err("Open Fairway and stop all services before deleting data.".into());
            }
            if layout.data.exists() {
                std::fs::remove_dir_all(&layout.data).map_err(|e| e.to_string())?;
            }
        }
        "--remove-coursework" => {
            if layout.www.exists() {
                std::fs::remove_dir_all(&layout.www).map_err(|e| e.to_string())?;
            }
        }
        "--prepare-uninstall" => (),
        _ => unreachable!(),
    }
    Ok(true)
}

pub fn run() {
    match maintenance(&std::env::args().collect::<Vec<_>>()) {
        Ok(true) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
        _ => (),
    }
    let shutting_down = Arc::new(AtomicBool::new(false));
    let shutdown_flag = shutting_down.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            if app.config().plugins.0.contains_key("updater") {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            let runtime = if cfg!(debug_assertions) {
                std::env::var_os("FAIRWAY_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime")
                    })
            } else {
                app.path().resource_dir()?.join("runtime")
            };
            let manager = Manager::new(Layout::new(runtime).map_err(std::io::Error::other)?)
                .map_err(std::io::Error::other)?;
            crate::config::atomic_write(
                &manager.layout.data.join("app.pid"),
                std::process::id().to_string().as_bytes(),
            )
            .map_err(std::io::Error::other)?;
            app.manage(Arc::new(Mutex::new(manager)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            service_action,
            save_ports,
            open_target,
            read_log,
            cli_integration,
            backup_database,
            check_update,
            install_update
        ])
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if shutdown_flag.swap(true, Ordering::SeqCst) {
                    return;
                }
                let app = window.app_handle().clone();
                let state = app.state::<Shared>().inner().clone();
                let flag = shutdown_flag.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = app.emit("fairway-shutdown", ());
                    match access(state, |m| {
                        m.stop_all()?;
                        let marker = m.layout.data.join("app.pid");
                        if marker.exists() {
                            std::fs::remove_file(marker).map_err(|e| e.to_string())?;
                        }
                        Ok(())
                    })
                    .await
                    {
                        Ok(()) => app.exit(0),
                        Err(e) => {
                            flag.store(false, Ordering::SeqCst);
                            let _ = app.emit("fairway-error", e);
                        }
                    }
                });
            }
        })
        .build(tauri::generate_context!())
        .expect("Fairway could not initialize");
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            if code == Some(tauri::RESTART_EXIT_CODE) {
                return;
            }
            if !shutting_down.load(Ordering::SeqCst) {
                api.prevent_exit();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.close();
                }
            }
        }
    });
}
