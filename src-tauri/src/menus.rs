//! Native popup menus (browser menu, tab and group context menus). Native menus are used because
//! they draw above website content, which Zuno's HTML interface cannot.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::menu::{CheckMenuItem, ContextMenu, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, LogicalPosition, Manager, Wry};

use crate::browser::navigation::InternalPage;
use crate::browser::tabs::{GroupId, TabId, GROUP_COLORS};
use crate::browser::{Browser, EVT_UI_ACTION};
use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MenuRequest {
    App { x: f64, y: f64 },
    Tab { tab: TabId, x: f64, y: f64 },
    Group { group: GroupId, x: f64, y: f64 },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum UiAction {
    RenameGroup { group: GroupId },
    About,
}

fn item(app: &AppHandle, id: String, text: &str, accel: Option<&str>) -> Result<MenuItem<Wry>> {
    Ok(MenuItem::with_id(app, id, text, true, accel)?)
}

fn sep(app: &AppHandle) -> Result<PredefinedMenuItem<Wry>> {
    Ok(PredefinedMenuItem::separator(app)?)
}

fn color_label(c: &str) -> String {
    let mut s = c.to_string();
    if let Some(f) = s.get_mut(0..1) {
        f.make_ascii_uppercase();
    }
    s
}

pub fn show(app: &AppHandle, browser: &Arc<Browser>, request: MenuRequest) -> Result<()> {
    let window = app.get_window(crate::MAIN_WINDOW).ok_or(Error::NotFound)?;
    let (menu, x, y) = match request {
        MenuRequest::App { x, y } => {
            let bar = browser.settings.get().show_bookmarks_bar;
            let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = vec![
                Box::new(item(app, "app.new-tab".into(), "New tab", Some("Ctrl+T"))?),
                Box::new(item(app, "app.incognito".into(), "New incognito window", Some("Ctrl+Shift+N"))?),
                Box::new(item(app, "app.reopen".into(), "Reopen closed tab", Some("Ctrl+Shift+T"))?),
                Box::new(sep(app)?),
                Box::new(item(app, "app.bookmarks".into(), "Bookmarks", Some("Ctrl+Shift+O"))?),
                Box::new(CheckMenuItem::with_id(app, "app.toggle-bar", "Show bookmarks bar", true, bar, Some("Ctrl+Shift+B"))?),
                Box::new(item(app, "app.history".into(), "History", Some("Ctrl+H"))?),
                Box::new(item(app, "app.downloads".into(), "Downloads", Some("Ctrl+J"))?),
                Box::new(sep(app)?),
            ];
            if !browser.is_incognito() {
                items.push(Box::new(item(app, "app.import".into(), "Import browser data…", None)?));
            }
            items.extend([
                Box::new(item(app, "app.devtools".into(), "Developer tools", Some("F12"))?) as Box<dyn IsMenuItem<Wry>>,
                Box::new(item(app, "app.settings".into(), "Settings", Some("Ctrl+,"))?),
                Box::new(item(app, "app.about".into(), "About Zuno", None)?),
                Box::new(sep(app)?),
                Box::new(item(app, "app.quit".into(), if browser.is_incognito() { "Close incognito window" } else { "Exit" }, None)?),
            ]);
            let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
            (Menu::with_items(app, &refs)?, x, y)
        }
        MenuRequest::Tab { tab, x, y } => {
            let snapshot = browser.snapshot();
            let current = snapshot.tabs.iter().find(|t| t.id == tab).ok_or(Error::NotFound)?;
            let mut group_items: Vec<Box<dyn IsMenuItem<Wry>>> = vec![Box::new(item(app, format!("tab.new-group:{tab}"), "New group", None)?)];
            if !snapshot.groups.is_empty() {
                group_items.push(Box::new(sep(app)?));
            }
            for g in &snapshot.groups {
                if current.group != Some(g.id) {
                    group_items.push(Box::new(item(app, format!("tab.group:{tab}:{}", g.id), &g.name, None)?));
                }
            }
            let group_refs: Vec<&dyn IsMenuItem<Wry>> = group_items.iter().map(|b| b.as_ref()).collect();
            let add_to_group = Submenu::with_items(app, "Add to group", true, &group_refs)?;

            let mut items: Vec<Box<dyn IsMenuItem<Wry>>> = vec![
                Box::new(item(app, format!("tab.new-right:{tab}"), "New tab to the right", None)?),
                Box::new(item(app, format!("tab.reload:{tab}"), "Reload", Some("Ctrl+R"))?),
                Box::new(item(app, format!("tab.duplicate:{tab}"), "Duplicate", None)?),
                Box::new(sep(app)?),
                Box::new(add_to_group),
            ];
            if current.group.is_some() {
                items.push(Box::new(item(app, format!("tab.ungroup:{tab}"), "Remove from group", None)?));
            }
            items.push(Box::new(sep(app)?));
            items.push(Box::new(item(app, format!("tab.close:{tab}"), "Close tab", Some("Ctrl+W"))?));
            if snapshot.tabs.len() > 1 {
                items.push(Box::new(item(app, format!("tab.close-others:{tab}"), "Close other tabs", None)?));
            }
            let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
            (Menu::with_items(app, &refs)?, x, y)
        }
        MenuRequest::Group { group, x, y } => {
            let snapshot = browser.snapshot();
            let g = snapshot.groups.iter().find(|g| g.id == group).ok_or(Error::NotFound)?;
            let colors: Vec<CheckMenuItem<Wry>> = GROUP_COLORS
                .iter()
                .map(|c| CheckMenuItem::with_id(app, format!("group.color:{group}:{c}"), color_label(c), true, g.color == *c, None::<&str>))
                .collect::<tauri::Result<_>>()?;
            let color_refs: Vec<&dyn IsMenuItem<Wry>> = colors.iter().map(|c| c as &dyn IsMenuItem<Wry>).collect();
            let color_menu = Submenu::with_items(app, "Colour", true, &color_refs)?;
            let items: Vec<Box<dyn IsMenuItem<Wry>>> = vec![
                Box::new(item(app, format!("group.rename:{group}"), "Rename group", None)?),
                Box::new(color_menu),
                Box::new(item(app, format!("group.collapse:{group}"), if g.collapsed { "Expand group" } else { "Collapse group" }, None)?),
                Box::new(item(app, format!("group.new-tab:{group}"), "New tab in group", None)?),
                Box::new(sep(app)?),
                Box::new(item(app, format!("group.ungroup:{group}"), "Ungroup", None)?),
                Box::new(item(app, format!("group.close:{group}"), "Close group", None)?),
            ];
            let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|b| b.as_ref()).collect();
            (Menu::with_items(app, &refs)?, x, y)
        }
    };
    menu.popup_at(window, LogicalPosition::new(x, y))?;
    Ok(())
}

fn parse<T: std::str::FromStr>(s: Option<&str>) -> Option<T> {
    s.and_then(|v| v.parse().ok())
}

/// Handles a native menu selection on a worker thread.
pub fn handle(app: &AppHandle, event: MenuEvent) {
    let Some(browser) = app.try_state::<Arc<Browser>>().map(|s| s.inner().clone()) else { return };
    let id = event.id().0.clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = dispatch(&app, &browser, &id) {
            tracing::warn!(menu = %id, error = %e, "menu action failed");
            browser.notice("error", e.to_string());
        }
    });
}

fn dispatch(app: &AppHandle, b: &Arc<Browser>, id: &str) -> Result<()> {
    let mut parts = id.split(':');
    let action = parts.next().unwrap_or_default();
    let a1 = parts.next();
    let a2 = parts.next();
    match action {
        "app.new-tab" => b.new_tab(None, None, true).map(|_| ()),
        "app.incognito" => crate::incognito::open_window(),
        "app.reopen" => b.reopen_closed().map(|_| ()),
        "app.bookmarks" => b.open_page(InternalPage::Bookmarks).map(|_| ()),
        "app.history" => b.open_page(InternalPage::History).map(|_| ()),
        "app.downloads" => b.open_page(InternalPage::Downloads).map(|_| ()),
        "app.import" => b.open_page(InternalPage::Import).map(|_| ()),
        "app.settings" => b.open_page(InternalPage::Settings).map(|_| ()),
        "app.toggle-bar" => {
            let saved = b.settings.update(|s| s.show_bookmarks_bar = !s.show_bookmarks_bar)?;
            b.apply_settings(&saved)
        }
        "app.devtools" => {
            let active = b.active_tab().filter(|t| t.live).ok_or_else(|| Error::msg("Open a website to use developer tools"))?;
            if let Some(webview) = app.get_webview(&format!("tab-{}", active.id)) {
                webview.open_devtools();
            }
            Ok(())
        }
        "app.about" => {
            b.emit(EVT_UI_ACTION, UiAction::About);
            Ok(())
        }
        "app.quit" => {
            b.on_exit();
            app.exit(0);
            Ok(())
        }
        "tab.new-right" => b.new_tab(None, parse(a1), true).map(|_| ()),
        "tab.reload" => b.reload(parse(a1).ok_or(Error::NotFound)?),
        "tab.duplicate" => b.duplicate(parse(a1).ok_or(Error::NotFound)?).map(|_| ()),
        "tab.close" => b.close_tab(parse(a1).ok_or(Error::NotFound)?),
        "tab.close-others" => b.close_other_tabs(parse(a1).ok_or(Error::NotFound)?),
        "tab.ungroup" => b.group_assign(parse(a1).ok_or(Error::NotFound)?, None),
        "tab.group" => b.group_assign(parse(a1).ok_or(Error::NotFound)?, parse(a2)),
        "tab.new-group" => {
            let group = b.group_create("New group", &[parse(a1).ok_or(Error::NotFound)?])?;
            b.emit(EVT_UI_ACTION, UiAction::RenameGroup { group });
            Ok(())
        }
        "group.rename" => {
            b.emit(EVT_UI_ACTION, UiAction::RenameGroup { group: parse(a1).ok_or(Error::NotFound)? });
            Ok(())
        }
        "group.color" => b.group_set_color(parse(a1).ok_or(Error::NotFound)?, a2.unwrap_or_default()),
        "group.collapse" => {
            let group: GroupId = parse(a1).ok_or(Error::NotFound)?;
            let collapsed = b.snapshot().groups.iter().find(|g| g.id == group).map(|g| g.collapsed).unwrap_or(false);
            b.group_collapse(group, !collapsed)
        }
        "group.new-tab" => {
            let group: GroupId = parse(a1).ok_or(Error::NotFound)?;
            let last = b.snapshot().tabs.iter().rev().find(|t| t.group == Some(group)).map(|t| t.id);
            b.new_tab(None, last, true).map(|_| ())
        }
        "group.ungroup" => b.group_remove(parse(a1).ok_or(Error::NotFound)?, false),
        "group.close" => b.group_remove(parse(a1).ok_or(Error::NotFound)?, true),
        _ => Ok(()),
    }
}
