//! The mods screen of the iOS build (which has no launcher): a "Get mods"
//! button on the title screen opens Horizon's registry (playhorizon.dev/mods)
//! over it, in tabs for mods, resource packs and shader packs. A tap on Get
//! downloads, checks and installs the file (`horizon_client::mod_registry`);
//! native mods are listed as PC only. New mods load the next time Horizon
//! starts. Touch only: it lives beside the touch controls (touch.rs).
use super::*;
use horizon_client::mod_registry::{self, Message, Offer, Registry, Tab};
use horizon_client_input::touch::Area;
use std::collections::HashMap;
use std::sync::Arc;
use winit::event::{Touch, TouchPhase};

const ROW: f32 = 34.0;
const BUTTON_W: f32 = 58.0;
const BAR: f32 = 20.0;
const GAP: f32 = 4.0;
const PANEL: u32 = 0xf010_1216;
const ROW_FACE: u32 = 0x60ff_ffff;
const BUTTON: u32 = 0xff30_8040;
const BUTTON_OFF: u32 = 0xff40_4448;
const TAB_ON: u32 = 0xff50_5aa0;
const INK: u32 = 0xffff_ffff;
const DIM: u32 = 0xffa0_a4ac;
const BAD: u32 = 0xffff_7070;

/// One download's state.
#[derive(Clone, Debug, PartialEq)]
enum Job {
    Downloading(f32),
    Done,
    Failed(String),
}

/// The open mods screen.
pub(super) struct ModBrowser {
    registry: Registry,
    index: Option<std::result::Result<Arc<horizon_mods::index::Index>, String>>,
    tab: Tab,
    scroll: f32,
    jobs: HashMap<String, Job>,
    /// A finger on the list: its id, where it was last, how far it went.
    drag: Option<(u64, f32, f32)>,
    /// Something was installed: say Horizon must restart.
    restart: bool,
}

impl ModBrowser {
    fn open() -> Self {
        let registry = Registry::default();
        registry.fetch_index();
        Self {
            registry,
            index: None,
            tab: Tab::Mods,
            scroll: 0.0,
            jobs: HashMap::new(),
            drag: None,
            restart: false,
        }
    }
}

/// Where its parts are, in GUI units.
struct Frame {
    panel: Area,
    close: Area,
    tabs: Vec<(Tab, Area)>,
    list: Area,
    footer_y: f32,
}

fn frame(safe: Area) -> Frame {
    let panel = Area {
        x: safe.x + GAP,
        y: safe.y + GAP,
        w: safe.w - GAP * 2.0,
        h: safe.h - GAP * 2.0,
    };
    let close = Area {
        x: panel.x + panel.w - 28.0 - GAP,
        y: panel.y + GAP,
        w: 28.0,
        h: BAR,
    };
    let tab_y = panel.y + GAP + BAR + GAP;
    let tab_w = 92.0;
    let tabs = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let area = Area {
                x: panel.x + GAP + i as f32 * (tab_w + GAP),
                y: tab_y,
                w: tab_w,
                h: BAR,
            };
            (*t, area)
        })
        .collect();
    let list_y = tab_y + BAR + GAP;
    let footer_h = 12.0;
    let list = Area {
        x: panel.x + GAP,
        y: list_y,
        w: panel.w - GAP * 2.0,
        h: (panel.y + panel.h - footer_h - GAP - list_y).max(ROW),
    };
    Frame {
        panel,
        close,
        tabs,
        list,
        footer_y: panel.y + panel.h - footer_h,
    }
}

/// A row's Get button within the list.
fn button(list: Area, row_y: f32) -> Area {
    Area {
        x: list.x + list.w - BUTTON_W - GAP,
        y: row_y + (ROW - BAR) / 2.0,
        w: BUTTON_W,
        h: BAR,
    }
}

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
impl Game<'_> {
    fn mods_safe(&self, batch: &HudBatch) -> Area {
        self.touch_safe(batch.width, batch.height, batch.scale)
    }

    /// The title screen's "Get mods" button, top right (iOS).
    fn get_mods_button(&self, batch: &HudBatch) -> Option<Area> {
        if !super::touch::TOUCH_CONTROLS
            || self.title.is_none()
            || self.screen == Screen::Options
            || self.pad.mod_browser.is_some()
        {
            return None;
        }
        let safe = self.mods_safe(batch);
        Some(Area {
            x: safe.x + safe.w - 70.0 - GAP,
            y: safe.y + GAP,
            w: 70.0,
            h: BAR,
        })
    }

    /// A touch for the mods screen or its button: `true` when it took it.
    pub(super) fn mods_touch(&mut self, touch: &Touch) -> bool {
        let Some(batch) = self.gui_scale() else {
            return false;
        };
        let at = (
            touch.location.x as f32 / batch.scale,
            touch.location.y as f32 / batch.scale,
        );
        if self.pad.mod_browser.is_none() {
            if touch.phase == TouchPhase::Started
                && self.get_mods_button(&batch).is_some_and(|b| b.contains(at))
            {
                log::info!("Horizon: opening the mods screen");
                self.pad.mod_browser = Some(ModBrowser::open());
                return true;
            }
            return false;
        }
        let f = frame(self.mods_safe(&batch));
        let game_dir = horizon_client::logging::player_dir();
        let Some(browser) = self.pad.mod_browser.as_mut() else {
            return false;
        };
        match touch.phase {
            TouchPhase::Started => {
                if f.close.contains(at) {
                    self.pad.mod_browser = None;
                    return true;
                }
                if let Some((tab, _)) = f.tabs.iter().find(|(_, a)| a.contains(at)) {
                    browser.tab = *tab;
                    browser.scroll = 0.0;
                    return true;
                }
                if matches!(browser.index, Some(Err(_))) && f.list.contains(at) {
                    browser.index = None;
                    browser.registry.fetch_index();
                    return true;
                }
                if f.list.contains(at) {
                    browser.drag = Some((touch.id, at.1, 0.0));
                }
            }
            TouchPhase::Moved => {
                if let Some((id, last, moved)) = &mut browser.drag
                    && *id == touch.id
                {
                    browser.scroll -= at.1 - *last;
                    *moved += (at.1 - *last).abs();
                    *last = at.1;
                }
                let rows = match &browser.index {
                    Some(Ok(index)) => mod_registry::entries(index, browser.tab).len(),
                    _ => 0,
                };
                let max = (rows as f32 * ROW - f.list.h).max(0.0);
                browser.scroll = browser.scroll.clamp(0.0, max);
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let Some((id, _, moved)) = browser.drag.take() else {
                    return true;
                };
                if id != touch.id || moved > 6.0 || touch.phase == TouchPhase::Cancelled {
                    return true;
                }
                // A tap on a row's button.
                let Some(Ok(index)) = browser.index.clone() else {
                    return true;
                };
                let entries = mod_registry::entries(&index, browser.tab);
                let row = ((at.1 - f.list.y + browser.scroll) / ROW).floor();
                if row < 0.0 {
                    return true;
                }
                let Some(project) = entries.get(row as usize) else {
                    return true;
                };
                let row_y = f.list.y + row * ROW - browser.scroll;
                if !button(f.list, row_y).contains(at) {
                    return true;
                }
                let busy = matches!(browser.jobs.get(&project.id), Some(Job::Downloading(_)));
                if !busy && let Offer::Get(_) = mod_registry::offer(&index, project, &game_dir) {
                    log::info!("Horizon: getting {}", project.id);
                    browser
                        .jobs
                        .insert(project.id.clone(), Job::Downloading(0.0));
                    browser
                        .registry
                        .install(index.clone(), project.id.clone(), game_dir);
                }
            }
        }
        true
    }

    /// Once a frame: what the worker reported.
    pub(super) fn mods_poll(&mut self) {
        let Some(browser) = self.pad.mod_browser.as_mut() else {
            return;
        };
        for message in browser.registry.poll() {
            match message {
                Message::Index(index) => {
                    if let Err(e) = &index {
                        log::warn!("Horizon: the mod registry: {e}");
                    }
                    browser.index = Some(index.map(Arc::new));
                }
                Message::Progress { id, done, total } => {
                    let k = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    browser.jobs.insert(id, Job::Downloading(k.min(1.0)));
                }
                Message::Installed { id, .. } => {
                    browser.jobs.insert(id, Job::Done);
                    browser.restart = true;
                }
                Message::Failed { id, error } => {
                    log::warn!("Horizon: installing {id}: {error}");
                    browser.jobs.insert(id, Job::Failed(error));
                }
            }
        }
    }

    /// The title screen's button, or the mods screen over everything.
    pub(super) fn draw_mods(&self, batch: &mut HudBatch) {
        let atlas = &self.gui.atlas;
        if let Some(b) = self.get_mods_button(batch) {
            batch.fill(atlas, b.x, b.y, b.w, b.h, BUTTON);
            centred(
                batch,
                atlas,
                &horizon_client::lang::get("horizon.mods.get_mods"),
                b,
                INK,
            );
        }
        let Some(browser) = self.pad.mod_browser.as_ref() else {
            return;
        };
        let f = frame(self.mods_safe(batch));
        let p = f.panel;
        batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, 0x8000_0000);
        batch.fill(atlas, p.x, p.y, p.w, p.h, PANEL);
        batch.text(
            atlas,
            &horizon_client::lang::get("horizon.mods.title"),
            p.x + GAP * 2.0,
            p.y + GAP + 6.0,
            INK,
            true,
        );
        batch.fill(
            atlas, f.close.x, f.close.y, f.close.w, f.close.h, BUTTON_OFF,
        );
        centred(
            batch,
            atlas,
            &horizon_client::lang::get("horizon.mods.close"),
            f.close,
            INK,
        );
        for (tab, a) in &f.tabs {
            let face = if *tab == browser.tab {
                TAB_ON
            } else {
                BUTTON_OFF
            };
            batch.fill(atlas, a.x, a.y, a.w, a.h, face);
            centred(
                batch,
                atlas,
                &horizon_client::lang::get(tab.label()),
                *a,
                INK,
            );
        }
        let list = f.list;
        let status = match &browser.index {
            None => Some((horizon_client::lang::get("horizon.mods.loading"), DIM)),
            Some(Err(e)) => Some((
                horizon_client::lang::get("horizon.mods.failed").replace("{error}", e),
                BAD,
            )),
            Some(Ok(_)) => None,
        };
        if let Some((text, colour)) = status {
            let text = fit(atlas, &text, list.w - GAP * 2.0);
            batch.text(atlas, &text, list.x + GAP, list.y + GAP, colour, true);
        }
        if let Some(Ok(index)) = &browser.index {
            let game_dir = horizon_client::logging::player_dir();
            let entries = mod_registry::entries(index, browser.tab);
            let max = (entries.len() as f32 * ROW - list.h).max(0.0);
            let scroll = browser.scroll.clamp(0.0, max);
            let first = (scroll / ROW).floor().max(0.0) as usize;
            let last = (((scroll + list.h) / ROW).ceil() as usize).min(entries.len());
            for (i, project) in entries.iter().enumerate().take(last).skip(first) {
                let y = list.y + i as f32 * ROW - scroll;
                // Rows only inside the list: no clipping in the HUD batch.
                if y < list.y - 0.5 || y + ROW > list.y + list.h + 0.5 {
                    continue;
                }
                batch.fill(atlas, list.x, y + ROW - 1.0, list.w, 1.0, ROW_FACE);
                let text_w = list.w - BUTTON_W - GAP * 4.0;
                let name = fit(atlas, &project.name, text_w);
                batch.text(atlas, &name, list.x + GAP, y + 3.0, INK, true);
                let by = horizon_client::lang::get("horizon.mods.by")
                    .replace("{authors}", &project.authors.join(", "))
                    .replace("{downloads}", &project.downloads.to_string());
                let by = fit(atlas, &by, text_w);
                batch.text(atlas, &by, list.x + GAP, y + 13.0, DIM, false);
                let summary = fit(atlas, &project.summary, text_w);
                batch.text(atlas, &summary, list.x + GAP, y + 23.0, DIM, false);
                let b = button(list, y);
                let (label, face) = match browser.jobs.get(&project.id) {
                    Some(Job::Downloading(k)) => (
                        horizon_client::lang::get("horizon.mods.downloading")
                            .replace("{percent}", &format!("{:.0}", k * 100.0)),
                        BUTTON_OFF,
                    ),
                    Some(Job::Done) => (
                        horizon_client::lang::get("horizon.mods.installed"),
                        BUTTON_OFF,
                    ),
                    Some(Job::Failed(_)) => (horizon_client::lang::get("horizon.mods.retry"), BAD),
                    None => match mod_registry::offer(index, project, &game_dir) {
                        Offer::Get(_) => (horizon_client::lang::get("horizon.mods.get"), BUTTON),
                        Offer::Installed => (
                            horizon_client::lang::get("horizon.mods.installed"),
                            BUTTON_OFF,
                        ),
                        Offer::PcOnly => (
                            horizon_client::lang::get("horizon.mods.pc_only"),
                            BUTTON_OFF,
                        ),
                        Offer::Unavailable => (
                            horizon_client::lang::get("horizon.mods.unavailable"),
                            BUTTON_OFF,
                        ),
                    },
                };
                batch.fill(atlas, b.x, b.y, b.w, b.h, face);
                centred(batch, atlas, &label, b, INK);
            }
        }
        let footer = if browser.restart {
            horizon_client::lang::get("horizon.mods.restart")
        } else {
            horizon_client::lang::get("horizon.mods.footer")
        };
        let footer = fit(atlas, &footer, p.w - GAP * 4.0);
        batch.text(atlas, &footer, p.x + GAP * 2.0, f.footer_y, DIM, false);
    }
}

/// `text` centred in `a`.
fn centred(batch: &mut HudBatch, atlas: &HudAtlas, text: &str, a: Area, ink: u32) {
    let text = fit(atlas, text, a.w - 4.0);
    let w = HudBatch::text_width(atlas, &text);
    batch.text(
        atlas,
        &text,
        a.x + (a.w - w) / 2.0,
        a.y + (a.h - 8.0) / 2.0,
        ink,
        true,
    );
}

/// `text` cut to `width`, keeping whole characters.
fn fit(atlas: &HudAtlas, text: &str, width: f32) -> String {
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if HudBatch::text_width(atlas, &out) > width {
            out.pop();
            break;
        }
    }
    out
}
