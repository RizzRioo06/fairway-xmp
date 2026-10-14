use super::Status;
use crate::{desktop, lifecycle::Lifecycle, manager::Snapshot};
use muda::{ContextMenu, Menu, MenuItem, PredefinedMenuItem, Submenu};
use objc2::{rc::Retained, AllocAnyThread};
use objc2_app_kit::{
    NSBaselineOffsetAttributeName, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSMenu,
};
use objc2_foundation::{NSMutableAttributedString, NSNumber, NSRange, NSString};
use std::cell::RefCell;
use tauri::{
    tray::{TrayIcon, TrayIconBuilder},
    Manager as _,
};

thread_local! {
    // Muda/AppKit objects never leave the main thread. Tauri dispatches every
    // access through with_inner_tray_icon rather than sending native pointers.
    static MENU: RefCell<Option<MacMenu>> = const { RefCell::new(None) };
}

struct Service {
    id: &'static str,
    name: &'static str,
    row: isize,
    start: MenuItem,
    stop: MenuItem,
    status: Status,
}

struct MacMenu {
    menu: Menu,
    start_all: MenuItem,
    stop_all: MenuItem,
    admin: MenuItem,
    quit: MenuItem,
    services: Vec<Service>,
    overall: Status,
}

impl MacMenu {
    fn new() -> Result<Self, String> {
        let show = MenuItem::with_id("show", "Show Fairway XMP", true, None);
        let start_all = MenuItem::with_id("start_all", "Start all", false, None);
        let stop_all = MenuItem::with_id("stop_all", "Stop all", false, None);
        let mut services = Vec::new();
        let mut submenus = Vec::new();
        for (id, name, row) in [("nginx", "Nginx", 5), ("mysql", "MySQL", 6)] {
            let start = MenuItem::with_id(format!("start:{id}"), "Start", false, None);
            let stop = MenuItem::with_id(format!("stop:{id}"), "Stop", false, None);
            submenus.push(
                Submenu::with_items(name, true, &[&start, &stop]).map_err(|e| e.to_string())?,
            );
            services.push(Service {
                id,
                name,
                row,
                start,
                stop,
                status: Status::Stopped,
            });
        }
        let admin = MenuItem::with_id("phpmyadmin", "Open phpMyAdmin", false, None);
        let quit = MenuItem::with_id(
            "quit",
            "Quit",
            true,
            Some(
                "Cmd+Q"
                    .parse()
                    .map_err(|e| format!("Invalid Quit shortcut: {e}"))?,
            ),
        );
        let menu = Menu::with_items(&[
            &show,
            &PredefinedMenuItem::separator(),
            &start_all,
            &stop_all,
            &PredefinedMenuItem::separator(),
            &submenus[0],
            &submenus[1],
            &admin,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;
        let result = Self {
            menu,
            start_all,
            stop_all,
            admin,
            quit,
            services,
            overall: Status::Stopped,
        };
        result.paint()?;
        Ok(result)
    }

    fn paint(&self) -> Result<(), String> {
        // SAFETY: Muda documents this NSMenu pointer as valid while its Menu is
        // alive. This function executes on the main thread and retains it for
        // the duration of the update; no native objects cross threads.
        let menu = unsafe { Retained::retain(self.menu.ns_menu().cast::<NSMenu>()) }
            .ok_or("The native tray menu is unavailable.")?;
        for (row, label, status) in [
            (
                self.services[0].row,
                self.services[0].name,
                self.services[0].status,
            ),
            (
                self.services[1].row,
                self.services[1].name,
                self.services[1].status,
            ),
        ] {
            let item = menu
                .itemAtIndex(row)
                .ok_or("A tray menu item is unavailable.")?;
            let text = NSString::from_str(&format!("{label}  ●"));
            let title = NSMutableAttributedString::initWithString(
                NSMutableAttributedString::alloc(),
                &text,
            );
            let dot = NSRange::new(text.length() - 1, 1);
            let (red, green, blue) = status.rgb();
            let text_color = if item.isEnabled() {
                NSColor::labelColor()
            } else {
                NSColor::disabledControlTextColor()
            };
            // SAFETY: AppKit attribute keys accept these NSFont, NSColor, and
            // NSNumber values. The ranges cover valid UTF-16 indices in text.
            unsafe {
                title.addAttribute_value_range(
                    NSFontAttributeName,
                    &NSFont::menuFontOfSize(0.0),
                    NSRange::new(0, text.length()),
                );
                title.addAttribute_value_range(
                    NSForegroundColorAttributeName,
                    &text_color,
                    NSRange::new(0, text.length()),
                );
                title.addAttribute_value_range(
                    NSFontAttributeName,
                    &NSFont::menuFontOfSize(10.0),
                    dot,
                );
                title.addAttribute_value_range(
                    NSForegroundColorAttributeName,
                    &NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, 1.0),
                    dot,
                );
                title.addAttribute_value_range(
                    NSBaselineOffsetAttributeName,
                    &NSNumber::numberWithDouble(1.0),
                    dot,
                );
            }
            item.setTitle(&text);
            item.setAttributedTitle(Some(&title));
        }
        Ok(())
    }

    fn set_busy(&self, quitting: bool) -> Result<(), String> {
        self.start_all.set_enabled(false);
        self.stop_all.set_enabled(false);
        self.admin.set_enabled(false);
        for service in &self.services {
            service.start.set_enabled(false);
            service.stop.set_enabled(false);
        }
        self.quit.set_enabled(!quitting);
        self.quit
            .set_text(if quitting { "Quitting…" } else { "Quit" });
        self.paint()
    }

    fn refresh(&mut self, snapshot: &Snapshot) -> Result<(), String> {
        self.overall = Status::overall(snapshot);
        self.start_all
            .set_enabled(snapshot.runtime_ready && !matches!(self.overall, Status::Started));
        self.stop_all
            .set_enabled(!matches!(self.overall, Status::Stopped));
        for service in &mut self.services {
            if let Some(status) = snapshot.services.iter().find(|s| s.id == service.id) {
                service.status = Status::service(status.ready);
                service
                    .start
                    .set_enabled(snapshot.runtime_ready && !status.running);
                service.stop.set_enabled(status.running);
            }
        }
        self.admin.set_enabled(
            snapshot
                .services
                .iter()
                .any(|s| s.id == "phpmyadmin" && s.ready),
        );
        self.quit.set_enabled(true);
        self.quit.set_text("Quit");
        self.paint()
    }
}

pub(crate) struct NativeTray {
    icon: TrayIcon,
}

fn error(message: String) -> tauri::Error {
    tauri::Error::Io(std::io::Error::other(message))
}

fn tray_icon() -> tauri::image::Image<'static> {
    let mut rgba = vec![0; 44 * 44 * 4];
    for y in 0..44 {
        for x in 0..44 {
            let stem = (10..17).contains(&x) && (6..38).contains(&y);
            let middle = (17..31).contains(&x) && (20..27).contains(&y);
            let leaf = (6..14).contains(&y) && x >= 17 && x < 36 - (y - 6);
            if stem || middle || leaf {
                rgba[(y * 44 + x) * 4 + 3] = 255;
            }
        }
    }
    tauri::image::Image::new_owned(rgba, 44, 44)
}

impl NativeTray {
    pub fn install(app: &tauri::AppHandle) -> tauri::Result<()> {
        let icon = TrayIconBuilder::with_id("fairway")
            .icon(tray_icon())
            .icon_as_template(true)
            .tooltip("Fairway XMP")
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "show" => desktop::show_window(app),
                "quit" => desktop::request_quit(app),
                id @ ("start_all" | "stop_all" | "phpmyadmin" | "start:nginx" | "stop:nginx"
                | "start:mysql" | "stop:mysql") => desktop::tray_action(app, id),
                _ => (),
            })
            .build(app)?;
        icon.with_inner_tray_icon(|inner| {
            let menu = MacMenu::new()?;
            inner.set_menu(Some(Box::new(menu.menu.clone())));
            MENU.with(|storage| *storage.borrow_mut() = Some(menu));
            Ok::<(), String>(())
        })?
        .map_err(error)?;
        app.manage(Self { icon });
        Ok(())
    }

    pub fn set_busy(&self, quitting: bool) -> tauri::Result<()> {
        self.icon
            .with_inner_tray_icon(move |_| {
                MENU.with(|storage| {
                    storage
                        .borrow()
                        .as_ref()
                        .ok_or("The tray menu is unavailable.".to_string())?
                        .set_busy(quitting)
                })
            })?
            .map_err(error)
    }

    pub fn refresh(&self, snapshot: &Snapshot, lifecycle: &Lifecycle) -> tauri::Result<()> {
        if lifecycle.busy() {
            return self.set_busy(lifecycle.quitting());
        }
        let snapshot = snapshot.clone();
        self.icon
            .with_inner_tray_icon(move |_| {
                MENU.with(|storage| {
                    storage
                        .borrow_mut()
                        .as_mut()
                        .ok_or("The tray menu is unavailable.".to_string())?
                        .refresh(&snapshot)
                })
            })?
            .map_err(error)
    }
}
