//! The mods screen of the iOS build (which has no launcher): the "Get mods"
//! button on the options' Mods page opens Horizon's registry (playhorizon.dev/mods)
//! over it, in tabs for mods, native mods, resource packs and shader packs. A
//! tap on Get downloads, checks and installs the file
//! (`horizon_client::mod_registry`). New mods load the next time Horizon
//! starts. Native mods are ticked in the fork's "Native mods for iOS" issue
//! (a GitHub token pasted once), and Build has GitHub compile an `.ipa` with
//! them (packaging/ios/native-mods.md). Touch only: it lives beside the touch
//! controls (touch.rs).
use super::*;
use horizon_client::mod_registry::{self, Message, Offer, Registry, Tab, UserMod};
use horizon_client_input::touch::Area;
use std::collections::{BTreeSet, HashMap};
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
    native: Native,
    /// The player's own mods (the My mods tab), from the fork's releases.
    mine: Option<std::result::Result<Vec<UserMod>, String>>,
    mine_loading: bool,
}

/// The Native tab's link to the checklist issue.
#[derive(Default)]
struct Native {
    token: Option<String>,
    /// The issue's number and body.
    issue: Option<(u64, String)>,
    ticks: BTreeSet<String>,
    /// What to say over the list, and in which colour.
    status: Option<(String, u32)>,
    /// A request to GitHub is out.
    busy: bool,
}

/// Where the GitHub token is kept: the app's Library, not Documents (which the
/// Files app shows).
fn token_path() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(horizon_client::logging::player_dir)
        .join("Library")
        .join("horizon-github-token")
}

fn load_token() -> Option<String> {
    let token = std::fs::read_to_string(token_path()).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

/// A GitHub token: a fine-grained (`github_pat_`) or classic (`ghp_`) one.
fn plausible_token(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with("github_pat_") || t.starts_with("ghp_"))
        && t.len() >= 30
        && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
            native: Native {
                token: load_token(),
                ..Native::default()
            },
            mine: None,
            mine_loading: false,
        }
    }

    /// The My mods tab needs the fork's releases: ask GitHub for them, once.
    fn want_mine(&mut self) {
        if self.tab != Tab::Mine || self.mine.is_some() {
            return;
        }
        match mod_registry::IOS_REPO {
            Some(repo) => {
                self.mine = Some(Ok(Vec::new()));
                self.mine_loading = true;
                self.registry.fetch_mine(repo.to_owned());
            }
            None => self.mine = Some(Err(horizon_client::lang::get("horizon.mods.mine.no_repo"))),
        }
    }

    /// How many rows the open tab has.
    fn row_count(&self) -> usize {
        if self.tab == Tab::Mine {
            return self
                .mine
                .as_ref()
                .and_then(|m| m.as_ref().ok())
                .map_or(0, Vec::len);
        }
        match &self.index {
            Some(Ok(index)) => mod_registry::entries(index, self.tab).len(),
            _ => 0,
        }
    }

    /// The Native tab needs the issue: ask GitHub for it, once.
    fn want_issue(&mut self) {
        let n = &mut self.native;
        if self.tab != Tab::Native || n.issue.is_some() || n.busy {
            return;
        }
        let (Some(repo), Some(token)) = (mod_registry::IOS_REPO, n.token.clone()) else {
            return;
        };
        if n.status.as_ref().is_some_and(|(_, c)| *c == BAD) {
            return;
        }
        n.busy = true;
        n.status = Some((
            horizon_client::lang::get("horizon.mods.native.loading"),
            DIM,
        ));
        self.registry.load_issue(repo.to_owned(), token);
    }

    /// The Native tab's bar button: paste the token, or build the ticked mods.
    fn native_action(&mut self, clipboard: String) {
        let n = &mut self.native;
        if n.busy {
            return;
        }
        let get = horizon_client::lang::get;
        let Some(repo) = mod_registry::IOS_REPO else {
            n.status = Some((get("horizon.mods.native.no_repo"), BAD));
            return;
        };
        if n.token.is_none() {
            if !plausible_token(&clipboard) {
                n.status = Some((get("horizon.mods.native.copy_token"), BAD));
                return;
            }
            let token = clipboard.trim().to_owned();
            let path = token_path();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = std::fs::write(&path, &token) {
                log::warn!("Horizon: keeping the GitHub token: {e}");
            }
            n.token = Some(token);
            n.status = None;
            self.want_issue();
            return;
        }
        let (Some((number, body)), Some(token)) = (n.issue.clone(), n.token.clone()) else {
            n.status = None;
            self.want_issue();
            return;
        };
        if n.ticks.is_empty() {
            n.status = Some((get("horizon.mods.native.tick_some"), BAD));
            return;
        }
        let body = mod_registry::with_ticks(&body, &n.ticks, true);
        n.issue = Some((number, mod_registry::with_ticks(&body, &n.ticks, false)));
        n.busy = true;
        n.status = Some((get("horizon.mods.native.starting"), DIM));
        log::info!("Horizon: native build of {:?}", n.ticks);
        self.registry
            .save_issue(repo.to_owned(), token, number, body);
    }
}

/// The rows' area: the list, under the Native tab's bar there.
fn rows(f: &Frame, tab: Tab) -> Area {
    if tab == Tab::Native {
        Area {
            y: f.list.y + BAR + GAP,
            h: (f.list.h - BAR - GAP).max(ROW),
            ..f.list
        }
    } else {
        f.list
    }
}

/// The Native tab's button, at the start of its bar.
fn native_button(f: &Frame) -> Area {
    Area {
        x: f.list.x,
        y: f.list.y,
        w: 96.0,
        h: BAR,
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
    let count = Tab::ALL.len() as f32;
    let tab_w = ((panel.w - GAP * (count + 1.0)) / count).clamp(48.0, 80.0);
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

    /// The Mods page's Get Mods button (options.rs, startup_menu.rs): the
    /// mods screen opens over the options. Touch only (iOS).
    pub(super) fn open_mods(&mut self) {
        if !super::touch::TOUCH_CONTROLS || self.pad.mod_browser.is_some() {
            return;
        }
        log::info!("Horizon: opening the mods screen");
        self.pad.mod_browser = Some(ModBrowser::open());
    }

    /// A touch for the mods screen: `true` when it took it.
    pub(super) fn mods_touch(&mut self, touch: &Touch) -> bool {
        if self.pad.mod_browser.is_none() {
            return false;
        }
        let Some(batch) = self.gui_scale() else {
            return false;
        };
        let at = (
            touch.location.x as f32 / batch.scale,
            touch.location.y as f32 / batch.scale,
        );
        let f = frame(self.mods_safe(&batch));
        let game_dir = horizon_client::logging::player_dir();
        // The Native tab's button may paste the token: read the clipboard first.
        let on_native_button = touch.phase == TouchPhase::Started
            && self
                .pad
                .mod_browser
                .as_ref()
                .is_some_and(|b| b.tab == Tab::Native)
            && native_button(&f).contains(at);
        let pasted = if on_native_button {
            use horizon_client::clipboard::Clipboard;
            self.clipboard.get()
        } else {
            String::new()
        };
        let Some(browser) = self.pad.mod_browser.as_mut() else {
            return false;
        };
        if on_native_button {
            browser.native_action(pasted);
            return true;
        }
        let list = rows(&f, browser.tab);
        match touch.phase {
            TouchPhase::Started => {
                if f.close.contains(at) {
                    self.pad.mod_browser = None;
                    return true;
                }
                if let Some((tab, _)) = f.tabs.iter().find(|(_, a)| a.contains(at)) {
                    browser.tab = *tab;
                    browser.scroll = 0.0;
                    browser.want_issue();
                    browser.want_mine();
                    return true;
                }
                if browser.tab == Tab::Mine
                    && matches!(browser.mine, Some(Err(_)))
                    && mod_registry::IOS_REPO.is_some()
                    && list.contains(at)
                {
                    browser.mine = None;
                    browser.want_mine();
                    return true;
                }
                if browser.tab != Tab::Mine
                    && matches!(browser.index, Some(Err(_)))
                    && list.contains(at)
                {
                    browser.index = None;
                    browser.registry.fetch_index();
                    return true;
                }
                if list.contains(at) {
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
                let rows = browser.row_count();
                let max = (rows as f32 * ROW - list.h).max(0.0);
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
                if browser.tab == Tab::Mine {
                    let row = ((at.1 - list.y + browser.scroll) / ROW).floor();
                    let Some(Ok(mine)) = &browser.mine else {
                        return true;
                    };
                    if row < 0.0 {
                        return true;
                    }
                    let Some(m) = mine.get(row as usize).cloned() else {
                        return true;
                    };
                    let row_y = list.y + row * ROW - browser.scroll;
                    let busy = matches!(browser.jobs.get(&m.id), Some(Job::Downloading(_)));
                    if button(list, row_y).contains(at)
                        && !m.native
                        && !busy
                        && !m.installed(&game_dir)
                    {
                        log::info!("Horizon: getting my mod {}", m.id);
                        browser.jobs.insert(m.id.clone(), Job::Downloading(0.0));
                        browser.registry.install_mine(m, game_dir);
                    }
                    return true;
                }
                let Some(Ok(index)) = browser.index.clone() else {
                    return true;
                };
                let entries = mod_registry::entries(&index, browser.tab);
                let row = ((at.1 - list.y + browser.scroll) / ROW).floor();
                if row < 0.0 {
                    return true;
                }
                let Some(project) = entries.get(row as usize) else {
                    return true;
                };
                let row_y = list.y + row * ROW - browser.scroll;
                if browser.tab == Tab::Native {
                    // A tap anywhere on a native mod's row ticks or unticks it.
                    let n = &mut browser.native;
                    if n.issue.is_some() && !n.busy {
                        if !n.ticks.remove(&project.id) {
                            n.ticks.insert(project.id.clone());
                        }
                    } else if n.token.is_none() {
                        n.status = Some((
                            horizon_client::lang::get("horizon.mods.native.copy_token"),
                            DIM,
                        ));
                    }
                    return true;
                }
                if !button(list, row_y).contains(at) {
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
                Message::Mine(result) => {
                    if let Err(e) = &result {
                        log::warn!("Horizon: my mods: {e}");
                    }
                    browser.mine_loading = false;
                    browser.mine = Some(result);
                }
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
                Message::Issue(result) => {
                    let n = &mut browser.native;
                    n.busy = false;
                    match result {
                        Ok((number, body)) => {
                            n.ticks = mod_registry::ticked(&body);
                            n.issue = Some((number, body));
                            n.status = None;
                        }
                        Err(e) => {
                            log::warn!("Horizon: the native mods issue: {e}");
                            if e.contains("401") {
                                // A bad token: forget it, so the button pastes a new one.
                                n.token = None;
                                let _ = std::fs::remove_file(token_path());
                            }
                            n.status = Some((e, BAD));
                        }
                    }
                }
                Message::IssueSaved(result) => {
                    let n = &mut browser.native;
                    n.busy = false;
                    n.status = Some(match result {
                        Ok(()) => (
                            horizon_client::lang::get("horizon.mods.native.started"),
                            DIM,
                        ),
                        Err(e) => {
                            log::warn!("Horizon: starting the native build: {e}");
                            (e, BAD)
                        }
                    });
                }
            }
        }
        browser.want_issue();
    }

    /// The mods screen over everything.
    pub(super) fn draw_mods(&self, batch: &mut HudBatch) {
        let atlas = &self.gui.atlas;
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
        if browser.tab == Tab::Mine {
            self.draw_mine(batch, browser, list);
            let footer = if browser.restart {
                horizon_client::lang::get("horizon.mods.restart")
            } else {
                horizon_client::lang::get("horizon.mods.footer")
            };
            let footer = fit(atlas, &footer, p.w - GAP * 4.0);
            batch.text(atlas, &footer, p.x + GAP * 2.0, f.footer_y, DIM, false);
            return;
        }
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
        let list = if browser.tab == Tab::Native
            && browser.index.as_ref().is_some_and(|i| i.is_ok())
        {
            let n = &browser.native;
            let get = horizon_client::lang::get;
            let b = native_button(&f);
            let (label, face) = match (&n.token, &n.issue) {
                (None, _) => (get("horizon.mods.native.paste"), BUTTON),
                (Some(_), Some(_)) => (
                    get("horizon.mods.native.build").replace("{count}", &n.ticks.len().to_string()),
                    if n.ticks.is_empty() || n.busy {
                        BUTTON_OFF
                    } else {
                        BUTTON
                    },
                ),
                (Some(_), None) => (get("horizon.mods.native.retry"), BUTTON_OFF),
            };
            batch.fill(atlas, b.x, b.y, b.w, b.h, face);
            centred(batch, atlas, &label, b, INK);
            let (text, colour) = match &n.status {
                Some((text, colour)) => (text.clone(), *colour),
                None if n.token.is_none() => (get("horizon.mods.native.how"), DIM),
                None => (get("horizon.mods.native.trust"), DIM),
            };
            let x = b.x + b.w + GAP * 2.0;
            let text = fit(atlas, &text, list.x + list.w - x - GAP);
            batch.text(atlas, &text, x, b.y + (BAR - 8.0) / 2.0, colour, false);
            rows(&f, Tab::Native)
        } else {
            list
        };
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
                let (label, face) = if browser.tab == Tab::Native {
                    let get = horizon_client::lang::get;
                    if browser.native.ticks.contains(&project.id) {
                        (get("horizon.mods.native.ticked"), BUTTON)
                    } else {
                        (get("horizon.mods.native.tick"), BUTTON_OFF)
                    }
                } else {
                    match browser.jobs.get(&project.id) {
                        Some(Job::Downloading(k)) => (
                            horizon_client::lang::get("horizon.mods.downloading")
                                .replace("{percent}", &format!("{:.0}", k * 100.0)),
                            BUTTON_OFF,
                        ),
                        Some(Job::Done) => (
                            horizon_client::lang::get("horizon.mods.installed"),
                            BUTTON_OFF,
                        ),
                        Some(Job::Failed(_)) => {
                            (horizon_client::lang::get("horizon.mods.retry"), BAD)
                        }
                        None => match mod_registry::offer(index, project, &game_dir) {
                            Offer::Get(_) => {
                                (horizon_client::lang::get("horizon.mods.get"), BUTTON)
                            }
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
                    }
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

#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
impl Game<'_> {
    /// The My mods tab: the player's own mods, content ones to get, native ones to build.
    fn draw_mine(&self, batch: &mut HudBatch, browser: &ModBrowser, list: Area) {
        let atlas = &self.gui.atlas;
        let get = horizon_client::lang::get;
        let mine = match &browser.mine {
            None => None,
            Some(Ok(_)) if browser.mine_loading => None,
            Some(Ok(mine)) => Some(mine),
            Some(Err(e)) => {
                let text = fit(
                    atlas,
                    &get("horizon.mods.failed").replace("{error}", e),
                    list.w - GAP * 2.0,
                );
                batch.text(atlas, &text, list.x + GAP, list.y + GAP, BAD, true);
                return;
            }
        };
        let Some(mine) = mine else {
            batch.text(
                atlas,
                &get("horizon.mods.loading"),
                list.x + GAP,
                list.y + GAP,
                DIM,
                true,
            );
            return;
        };
        if mine.is_empty() {
            let text = fit(atlas, &get("horizon.mods.mine.none"), list.w - GAP * 2.0);
            batch.text(atlas, &text, list.x + GAP, list.y + GAP, DIM, true);
            return;
        }
        let game_dir = horizon_client::logging::player_dir();
        let max = (mine.len() as f32 * ROW - list.h).max(0.0);
        let scroll = browser.scroll.clamp(0.0, max);
        for (i, m) in mine.iter().enumerate() {
            let y = list.y + i as f32 * ROW - scroll;
            if y < list.y - 0.5 || y + ROW > list.y + list.h + 0.5 {
                continue;
            }
            batch.fill(atlas, list.x, y + ROW - 1.0, list.w, 1.0, ROW_FACE);
            let text_w = list.w - BUTTON_W - GAP * 4.0;
            batch.text(
                atlas,
                &fit(atlas, &m.id, text_w),
                list.x + GAP,
                y + 3.0,
                INK,
                true,
            );
            let kind = get(if m.native {
                "horizon.mods.mine.native"
            } else {
                "horizon.mods.mine.content"
            });
            let by = get("horizon.mods.mine.by")
                .replace("{version}", &m.version)
                .replace("{kind}", &kind);
            batch.text(
                atlas,
                &fit(atlas, &by, text_w),
                list.x + GAP,
                y + 13.0,
                DIM,
                false,
            );
            let summary = get(if m.native {
                "horizon.mods.mine.native_summary"
            } else {
                "horizon.mods.mine.summary"
            });
            batch.text(
                atlas,
                &fit(atlas, &summary, text_w),
                list.x + GAP,
                y + 23.0,
                DIM,
                false,
            );
            let b = button(list, y);
            let (label, face) = match browser.jobs.get(&m.id) {
                _ if m.native => (get("horizon.mods.mine.build"), BUTTON_OFF),
                Some(Job::Downloading(k)) => (
                    get("horizon.mods.downloading")
                        .replace("{percent}", &format!("{:.0}", k * 100.0)),
                    BUTTON_OFF,
                ),
                Some(Job::Failed(_)) => (get("horizon.mods.retry"), BAD),
                _ if m.installed(&game_dir) => (get("horizon.mods.installed"), BUTTON_OFF),
                _ => (get("horizon.mods.get"), BUTTON),
            };
            batch.fill(atlas, b.x, b.y, b.w, b.h, face);
            centred(batch, atlas, &label, b, INK);
        }
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
