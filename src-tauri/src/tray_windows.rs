use super::Status;
use crate::{desktop, lifecycle::Lifecycle, manager::Snapshot};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    Manager as _,
};

struct ServiceMenu {
    id: &'static str,
    name: &'static str,
    menu: Submenu<tauri::Wry>,
    start: MenuItem<tauri::Wry>,
    stop: MenuItem<tauri::Wry>,
}

pub(crate) struct NativeTray {
    start_all: MenuItem<tauri::Wry>,
    stop_all: MenuItem<tauri::Wry>,
    services: Vec<ServiceMenu>,
    admin: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

fn tray_icon(app: &tauri::AppHandle) -> tauri::Result<tauri::image::Image<'static>> {
    app.default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::Io(std::io::Error::other("The tray icon is missing.")))
}

impl NativeTray {
    pub fn install(app: &tauri::AppHandle) -> tauri::Result<()> {
        let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);
        let show = item("show", "Show Fairway XMP")?;
        let start_all = item("start_all", "Start all")?;
        let stop_all = item("stop_all", "Stop all")?;
        let mut services = Vec::new();
        for (id, name) in [("nginx", "Nginx"), ("mysql", "MySQL")] {
            let start = MenuItem::with_id(app, format!("start:{id}"), "Start", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, format!("stop:{id}"), "Stop", true, None::<&str>)?;
            let menu = Submenu::with_items(app, format!("{name} 🔴"), true, &[&start, &stop])?;
            services.push(ServiceMenu {
                id,
                name,
                menu,
                start,
                stop,
            });
        }
        let admin = item("phpmyadmin", "Open phpMyAdmin")?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let separator = PredefinedMenuItem::separator(app)?;
        let separator2 = PredefinedMenuItem::separator(app)?;
        let separator3 = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(
            app,
            &[
                &show,
                &separator,
                &start_all,
                &stop_all,
                &separator2,
                &services[0].menu,
                &services[1].menu,
                &admin,
                &separator3,
                &quit,
            ],
        )?;
        TrayIconBuilder::with_id("fairway")
            .icon(tray_icon(app)?)
            .tooltip("Fairway XMP")
            .menu(&menu)
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "show" => desktop::show_window(app),
                "quit" => desktop::request_quit(app),
                id @ ("start_all" | "stop_all" | "phpmyadmin" | "start:nginx" | "stop:nginx"
                | "start:mysql" | "stop:mysql") => desktop::tray_action(app, id),
                _ => (),
            })
            .build(app)?;
        let tray = Self {
            start_all,
            stop_all,
            services,
            admin,
            quit,
        };
        tray.set_busy(false)?;
        app.manage(tray);
        Ok(())
    }

    /// Disable operations until a fresh snapshot is available. Show remains
    /// usable and Quit may be requested while another operation finishes.
    pub fn set_busy(&self, quitting: bool) -> tauri::Result<()> {
        self.start_all.set_enabled(false)?;
        self.stop_all.set_enabled(false)?;
        self.admin.set_enabled(false)?;
        for service in &self.services {
            service.start.set_enabled(false)?;
            service.stop.set_enabled(false)?;
        }
        self.quit.set_enabled(!quitting)?;
        self.quit
            .set_text(if quitting { "Quitting…" } else { "Quit" })?;
        Ok(())
    }

    pub fn refresh(&self, snapshot: &Snapshot, lifecycle: &Lifecycle) -> tauri::Result<()> {
        if lifecycle.busy() {
            return self.set_busy(lifecycle.quitting());
        }
        let overall = Status::overall(snapshot);
        self.start_all
            .set_enabled(snapshot.runtime_ready && !matches!(overall, Status::Started))?;
        self.stop_all
            .set_enabled(!matches!(overall, Status::Stopped))?;
        for service in &self.services {
            if let Some(status) = snapshot.services.iter().find(|s| s.id == service.id) {
                service.menu.set_text(format!(
                    "{} {}",
                    service.name,
                    Status::service(status.ready).glyph()
                ))?;
                service
                    .start
                    .set_enabled(snapshot.runtime_ready && !status.running)?;
                service.stop.set_enabled(status.running)?;
            }
        }
        self.admin.set_enabled(
            snapshot
                .services
                .iter()
                .any(|s| s.id == "phpmyadmin" && s.ready),
        )?;
        self.quit.set_enabled(true)?;
        self.quit.set_text("Quit")?;
        Ok(())
    }
}
