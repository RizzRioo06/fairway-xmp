use super::Status;
use crate::{desktop, lifecycle::Lifecycle, manager::Snapshot};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu, SubmenuBuilder},
    tray::TrayIconBuilder,
    Manager as _,
};

struct ServiceMenu {
    id: &'static str,
    menu: Submenu<tauri::Wry>,
    start: MenuItem<tauri::Wry>,
    stop: MenuItem<tauri::Wry>,
    ready: std::sync::atomic::AtomicBool,
}

pub(crate) struct NativeTray {
    start_all: MenuItem<tauri::Wry>,
    stop_all: MenuItem<tauri::Wry>,
    services: Vec<ServiceMenu>,
    admin: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

fn tray_icon(app: &tauri::AppHandle) -> tauri::Result<tauri::image::Image<'static>> {
    let icon = app
        .default_window_icon()
        .ok_or_else(|| tauri::Error::Io(std::io::Error::other("The tray icon is missing.")))?;
    Ok(tauri::image::Image::new_owned(
        icon.rgba().to_vec(),
        icon.width(),
        icon.height(),
    ))
}

fn status_icon(ready: bool) -> tauri::image::Image<'static> {
    let color = if ready { [73, 160, 106] } else { [207, 83, 84] };
    let mut rgba = vec![0; 16 * 16 * 4];
    for y in 0..16 {
        for x in 0..16 {
            let mut coverage = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let dx = x as f32 + (sx as f32 + 0.5) / 4.0 - 8.0;
                    let dy = y as f32 + (sy as f32 + 0.5) / 4.0 - 8.0;
                    if dx * dx + dy * dy <= 12.25 {
                        coverage += 1;
                    }
                }
            }
            let pixel = (y * 16 + x) * 4;
            rgba[pixel..pixel + 4].copy_from_slice(&[
                color[0],
                color[1],
                color[2],
                (coverage * 255 / 16) as u8,
            ]);
        }
    }
    // Windows renders this RGBA image as a menu bitmap, independent of emoji
    // font support. A 7-pixel dot fits inside the native 16-pixel icon gutter.
    tauri::image::Image::new_owned(rgba, 16, 16)
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
            let menu = SubmenuBuilder::with_id(app, format!("service:{id}"), name)
                .submenu_icon(status_icon(false))
                .items(&[&start, &stop])
                .build()?;
            services.push(ServiceMenu {
                id,
                menu,
                start,
                stop,
                ready: std::sync::atomic::AtomicBool::new(false),
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
                if service.ready.load(std::sync::atomic::Ordering::Relaxed) != status.ready {
                    service.menu.set_icon(Some(status_icon(status.ready)))?;
                    service
                        .ready
                        .store(status.ready, std::sync::atomic::Ordering::Relaxed);
                }
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
