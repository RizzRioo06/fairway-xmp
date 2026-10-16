//! Real Tauri update installation on a disposable hosted runner; never shipped.
use fairway_core::{
    config::{Layout, Result},
    manager::Manager,
    updates,
};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri_plugin_updater::UpdaterExt;

async fn exercise(app: tauri::AppHandle, args: Vec<String>) -> Result<()> {
    let executable = PathBuf::from(&args[0]);
    let runtime = fairway_core::desktop::runtime_location(&executable)?;
    let mut manager = Manager::new(Layout::new(runtime)?)?;
    manager.start_all()?;
    let report = PathBuf::from(&args[2]);
    let result = async {
        for feed in ["bad.json", "wrong-version.json"] {
            let update = app.updater_builder()
                .executable_path(executable.clone())
                .endpoints(vec![format!("{}/{feed}", args[1]).parse().map_err(|e| format!("{e}"))?]).map_err(|e| e.to_string())?
                // Explicit same-version reinstall drill. Production uses the
                // plugin's normal semver comparison and rejects downgrades.
                .version_comparator(|_, _| true)
                .timeout(Duration::from_secs(120))
                .build().map_err(|e| e.to_string())?
                .check().await.map_err(|e| e.to_string())?.ok_or("Test update missing")?;
            assert!(update.download(|_, _| {}, || {}).await.is_err());
            assert!(manager.snapshot().services.iter().all(|s| s.running && s.ready));
        }
        let builder = app.updater_builder()
            .executable_path(executable.clone())
            .endpoints(vec![format!("{}/latest.json", args[1]).parse().map_err(|e| format!("{e}"))?]).map_err(|e| e.to_string())?
            .version_comparator(|_, _| true)
            .timeout(Duration::from_secs(120))
            .on_before_exit(|| {});
        #[cfg(windows)]
        let builder = builder.restart_after_install(false);
        let mut update = builder.build().map_err(|e| e.to_string())?
            .check().await.map_err(|e| e.to_string())?.ok_or("Test update missing")?;
        update.timeout = Some(Duration::from_secs(600));
        let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
        assert!(manager.snapshot().services.iter().all(|s| s.ready));
        let backups = fs::read_dir(manager.layout.data.join("backups")).map_err(|e| e.to_string())?.count();
        updates::prepare(&mut manager)?;
        assert!(manager.snapshot().services.iter().all(|s| !s.running));
        assert!(fs::read_dir(manager.layout.data.join("backups")).map_err(|e| e.to_string())?.count() > backups);
        fs::remove_file(manager.layout.data.join("app.pid")).map_err(|e| e.to_string())?;
        fs::write(&report, serde_json::to_vec_pretty(&serde_json::json!({
            "status": "prepared", "checks": ["tampered-download-keeps-services-running", "signed-version-mismatch-rejected", "verified-download-keeps-services-running", "database-backup-before-shutdown", "services-stopped-and-markers-cleared-before-install"],
        })).unwrap()).map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || update.install(bytes)).await.map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        Ok(())
    }.await;
    let stop = manager.stop_all();
    result.and(stop)
}

fn main() {
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
    assert_eq!(
        std::env::var("RUNNER_ENVIRONMENT").as_deref(),
        Ok("github-hosted")
    );
    assert_eq!(std::env::var("FAIRWAY_INSTALLER_TEST").as_deref(), Ok("1"));
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    assert_eq!(
        args.len(),
        4,
        "updater_probe INSTALLED_EXE FEED_BASE REPORT PUBLIC_KEY"
    );
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": fs::read_to_string(&args[3]).unwrap().trim(),
            "dangerousInsecureTransportProtocol": true,
            "requireSignedVersion": true,
            "windows": {"installMode": "quiet"},
        }),
    );
    let status = Arc::new(Mutex::new(None));
    let finished = status.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let app = app.handle().clone();
            let layout = Layout::new(
                fairway_core::desktop::runtime_location(&PathBuf::from(&args[0])).unwrap(),
            )
            .unwrap();
            fs::write(layout.data.join("app.pid"), std::process::id().to_string()).unwrap();
            tauri::async_runtime::spawn(async move {
                let result = exercise(app.clone(), args).await;
                if let Err(error) = &result {
                    eprintln!("{error}");
                }
                *finished.lock().unwrap() = Some(result.is_ok());
                app.exit(if result.is_ok() { 0 } else { 1 });
            });
            Ok(())
        })
        .run(context)
        .expect("updater probe runtime");
    if *status.lock().unwrap() != Some(true) {
        std::process::exit(1);
    }
}
