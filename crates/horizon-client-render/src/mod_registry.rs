//! Getting mods from Horizon's registry (playhorizon.dev/mods) inside the
//! game, for the iOS build, which has no launcher. It reads the registry's
//! `index.json` (docs/api/mod-index.md) and installs what is only files:
//! content mods (`.hmod`), resource packs and shader packs. Native mods are
//! Rust code compiled into the game, which an iPhone can't do: the Native tab
//! ticks them in the fork's "Native mods for iOS" issue, and ticking Build now
//! there has GitHub build an `.ipa` with them (packaging/ios/native-mods.md).
//!
//! The index and each download run on a worker thread and report over a
//! channel; the screen only polls. A file is checked against its entry
//! (`horizon_mods::index::check_file`: size, SHA-256, revocation, signature,
//! manifest) before it reaches the game folder, as the launcher does.
use horizon_mods::index::{Index, Kind, Project, Version, check_file};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        mpsc::{Receiver, Sender, channel},
    },
};

/// The registry's index, as the launcher reads it.
pub const INDEX_URL: &str = "https://horizon-rust.github.io/horizon-mods/index.json";
const AGENT: &str = concat!(
    "horizon-rust/horizon/",
    env!("CARGO_PKG_VERSION"),
    " (in-game mod browser)"
);
/// The index is a few hundred KiB; past this it isn't read.
const MAX_INDEX_BYTES: u64 = 16 * 1024 * 1024;
/// The registry caps a file at 64 MiB.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// The browser's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Mods,
    Native,
    ResourcePacks,
    Shaders,
    /// The player's own mods, made with Horizon Mods' mod maker ([`UserMod`]).
    Mine,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Mods,
        Tab::Native,
        Tab::ResourcePacks,
        Tab::Shaders,
        Tab::Mine,
    ];

    /// The translation key of its name.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Mods => "horizon.mods.tab.mods",
            Tab::Native => "horizon.mods.tab.native",
            Tab::ResourcePacks => "horizon.mods.tab.resourcepacks",
            Tab::Shaders => "horizon.mods.tab.shaders",
            Tab::Mine => "horizon.mods.tab.mine",
        }
    }

    fn holds(self, kind: &Kind) -> bool {
        match self {
            Tab::Mods => *kind == Kind::Mod,
            Tab::Native => *kind == Kind::NativeMod,
            Tab::ResourcePacks => *kind == Kind::ResourcePack,
            Tab::Shaders => matches!(kind, Kind::Shader | Kind::ShaderPack),
            Tab::Mine => false,
        }
    }
}

/// The folder beside `options.txt` a kind's file goes in; `None` for what
/// this game can't install (a native mod, a modpack, a data pack).
pub fn folder(kind: &Kind) -> Option<&'static str> {
    match kind {
        Kind::Mod => Some(crate::mod_list::DIR),
        Kind::ResourcePack => Some("resourcepacks"),
        Kind::Shader | Kind::ShaderPack => Some("shaderpacks"),
        _ => None,
    }
}

/// What the browser offers for one project.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Offer<'a> {
    /// Its newest version for this game can be installed.
    Get(&'a Version),
    /// That version's file is already in its folder.
    Installed,
    /// A native mod: only a launcher on a computer can build it in.
    PcOnly,
    /// No version for this Minecraft, or every one was taken down.
    Unavailable,
}

/// The projects on `tab`, most downloaded first.
pub fn entries(index: &Index, tab: Tab) -> Vec<&Project> {
    let mut out: Vec<&Project> = index
        .projects
        .iter()
        .filter(|p| tab.holds(&p.kind))
        .collect();
    out.sort_by(|a, b| {
        b.downloads
            .cmp(&a.downloads)
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// What can be done with `project` in the game folder `game_dir`.
pub fn offer<'a>(index: &'a Index, project: &'a Project, game_dir: &Path) -> Offer<'a> {
    if project.kind == Kind::NativeMod {
        return Offer::PcOnly;
    }
    let Some(folder) = folder(&project.kind) else {
        return Offer::Unavailable;
    };
    let Some(version) = index.installable(
        project,
        horizon_core::version::VERSION_ID,
        horizon_mods::api_level::LEVEL,
    ) else {
        return Offer::Unavailable;
    };
    if game_dir.join(folder).join(&version.file.name).is_file() {
        Offer::Installed
    } else {
        Offer::Get(version)
    }
}

/// The fork the app was built from (`owner/name`), which builds its native
/// mods: set by the iOS workflows (`HORIZON_IOS_REPO`).
pub const IOS_REPO: Option<&str> = option_env!("HORIZON_IOS_REPO");
/// The issue whose checklist picks the native mods (ios-native-list.yml).
pub const ISSUE_TITLE: &str = "Native mods for iOS";
/// The checklist's line that starts a build.
const BUILD_LINE: &str = "- [ ] **Build now**";

/// The native mod ids ticked in the issue's `body`.
pub fn ticked(body: &str) -> std::collections::BTreeSet<String> {
    body.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("- [")?;
            let (mark, rest) = rest.split_at_checked(1)?;
            let id = rest.strip_prefix("] `")?.split('`').next()?;
            matches!(mark, "x" | "X").then(|| id.to_owned())
        })
        .collect()
}

/// `body` with exactly `ticks` ticked, and Build now ticked when `build`.
pub fn with_ticks(body: &str, ticks: &std::collections::BTreeSet<String>, build: bool) -> String {
    let mut out = String::with_capacity(body.len());
    for line in body.lines() {
        let trimmed = line.trim();
        let new = if trimmed.eq_ignore_ascii_case(BUILD_LINE)
            || trimmed.eq_ignore_ascii_case("- [x] **Build now**")
        {
            if build {
                "- [x] **Build now**"
            } else {
                BUILD_LINE
            }
            .to_owned()
        } else if let Some(rest) = trimmed
            .strip_prefix("- [ ] `")
            .or_else(|| trimmed.strip_prefix("- [x] `"))
            .or_else(|| trimmed.strip_prefix("- [X] `"))
        {
            let id = rest.split('`').next().unwrap_or_default();
            let mark = if ticks.contains(id) { 'x' } else { ' ' };
            format!("- [{mark}] `{rest}")
        } else {
            line.to_owned()
        };
        out.push_str(&new);
        out.push('\n');
    }
    out
}

/// A mod of the player's own: one the fork's `ios-user-mod.yml` checked and published as the
/// release `mod-<id>-v<version>` (packaging/ios/mod-maker.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserMod {
    pub id: String,
    /// `content` (an `.hmod`, installed here) or `native` (built into the game on GitHub).
    pub native: bool,
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

impl UserMod {
    /// Where an installed content mod is.
    pub fn file(&self, game_dir: &Path) -> PathBuf {
        game_dir
            .join("horizon-mods")
            .join(format!("{}.hmod", self.id))
    }

    /// Its newest version is installed: the file is there at its size.
    pub fn installed(&self, game_dir: &Path) -> bool {
        std::fs::metadata(self.file(game_dir)).is_ok_and(|m| m.len() == self.size)
    }
}

const USER_MARK: &str = "<!-- horizon-user-mod ";

/// The player's own mods in a GitHub releases listing (JSON), the newest release of each id.
pub fn user_mods(releases: &str) -> Vec<UserMod> {
    let Ok(list) = serde_json::from_str::<Vec<serde_json::Value>>(releases) else {
        return Vec::new();
    };
    let mut newest: std::collections::BTreeMap<String, (String, UserMod)> = Default::default();
    for r in &list {
        let (Some(tag), Some(body)) = (r["tag_name"].as_str(), r["body"].as_str()) else {
            continue;
        };
        let Some(meta) = body
            .split_once(USER_MARK)
            .and_then(|(_, rest)| rest.split_once(" -->"))
            .and_then(|(json, _)| serde_json::from_str::<serde_json::Value>(json).ok())
        else {
            continue;
        };
        let (Some(id), Some(kind), Some(version), Some(sha256), Some(size)) = (
            meta["id"].as_str(),
            meta["kind"].as_str(),
            meta["version"].as_str(),
            meta["sha256"].as_str(),
            meta["size"].as_u64(),
        ) else {
            continue;
        };
        let file = format!("{id}.{}", if kind == "native" { "hnmod" } else { "hmod" });
        let Some(url) = r["assets"].as_array().and_then(|a| {
            a.iter()
                .find(|a| a["name"] == file.as_str())
                .and_then(|a| a["browser_download_url"].as_str())
        }) else {
            continue;
        };
        if !tag.starts_with("mod-") || !safe_name(&file) || !url.starts_with("https://") {
            continue;
        }
        let created = r["created_at"].as_str().unwrap_or("").to_owned();
        if newest.get(id).is_some_and(|(at, _)| *at >= created) {
            continue;
        }
        newest.insert(
            id.to_owned(),
            (
                created,
                UserMod {
                    id: id.to_owned(),
                    native: kind == "native",
                    version: version.to_owned(),
                    url: url.to_owned(),
                    sha256: sha256.to_lowercase(),
                    size,
                },
            ),
        );
    }
    newest.into_values().map(|(_, m)| m).collect()
}

/// Download a content mod of the player's own, check its size and SHA-256 against its release,
/// and put it in `horizon-mods/`, turned on.
fn install_mine(
    m: &UserMod,
    game_dir: &Path,
    progress: impl Fn(u64, u64),
) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    if m.native {
        return Err(format!("{} is a native mod: build it into Horizon", m.id));
    }
    let bytes = get(&m.url, m.size.min(MAX_FILE_BYTES), &progress)?;
    let sha: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if bytes.len() as u64 != m.size || sha != m.sha256 {
        return Err(format!("{}'s file isn't the one GitHub checked", m.id));
    }
    let to = m.file(game_dir);
    std::fs::create_dir_all(to.parent().expect("a file in horizon-mods"))
        .map_err(|e| e.to_string())?;
    let partial = to.with_extension("hmod.part");
    std::fs::write(&partial, &bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &to).map_err(|e| e.to_string())?;
    let mut list = crate::mod_list::ModList::load(game_dir);
    list.enabled.insert(m.id.clone());
    list.save(game_dir);
    log::info!("Horizon: installed my mod {} {}", m.id, m.version);
    Ok(format!("{}.hmod", m.id))
}

/// What the worker reports.
#[derive(Debug)]
pub enum Message {
    /// The player's own mods, from the fork's releases.
    Mine(Result<Vec<UserMod>, String>),
    /// The checklist issue: its number and body.
    Issue(Result<(u64, String), String>),
    /// The checklist was saved (with Build now ticked: a build started).
    IssueSaved(Result<(), String>),
    Index(Result<Index, String>),
    /// Bytes of `id`'s file read so far, of `total`.
    Progress {
        id: String,
        done: u64,
        total: u64,
    },
    /// `id` is installed as `file` in its folder.
    Installed {
        id: String,
        file: String,
    },
    Failed {
        id: String,
        error: String,
    },
}

/// The worker's channel.
pub struct Registry {
    tx: Sender<Message>,
    rx: Receiver<Message>,
}

impl Default for Registry {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx }
    }
}

impl Registry {
    /// Read the index (again).
    pub fn fetch_index(&self) {
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("mod index".into())
            .spawn(move || {
                let index = get(INDEX_URL, MAX_INDEX_BYTES, |_, _| {})
                    .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
                    .and_then(|text| Index::parse(&text));
                let _ = tx.send(Message::Index(index));
            })
            .ok();
    }

    /// Download, check and install `id` into `game_dir`.
    pub fn install(&self, index: Arc<Index>, id: String, game_dir: PathBuf) {
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("mod download".into())
            .spawn(move || {
                let progress = |done, total| {
                    let _ = tx.send(Message::Progress {
                        id: id.clone(),
                        done,
                        total,
                    });
                };
                let message = match install(&index, &id, &game_dir, progress) {
                    Ok(file) => Message::Installed { id, file },
                    Err(error) => Message::Failed { id, error },
                };
                let _ = tx.send(message);
            })
            .ok();
    }

    /// Read the player's own mods from `repo`'s releases (public: no token).
    pub fn fetch_mine(&self, repo: String) {
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("my mods".into())
            .spawn(move || {
                let url = format!("https://api.github.com/repos/{repo}/releases?per_page=100");
                let mine = get(&url, MAX_INDEX_BYTES, |_, _| {})
                    .and_then(|bytes| String::from_utf8(bytes).map_err(|e| e.to_string()))
                    .map(|text| user_mods(&text));
                let _ = tx.send(Message::Mine(mine));
            })
            .ok();
    }

    /// Download and install the player's own content mod `m` into `game_dir`.
    pub fn install_mine(&self, m: UserMod, game_dir: PathBuf) {
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("my mod download".into())
            .spawn(move || {
                let id = m.id.clone();
                let progress = |done, total| {
                    let _ = tx.send(Message::Progress {
                        id: id.clone(),
                        done,
                        total,
                    });
                };
                let message = match install_mine(&m, &game_dir, progress) {
                    Ok(file) => Message::Installed { id: m.id, file },
                    Err(error) => Message::Failed { id: m.id, error },
                };
                let _ = tx.send(message);
            })
            .ok();
    }

    /// Read the checklist issue of `repo` with `token`.
    pub fn load_issue(&self, repo: String, token: String) {
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("native mods issue".into())
            .spawn(move || {
                let _ = tx.send(Message::Issue(find_issue(&repo, &token)));
            })
            .ok();
    }

    /// Write the checklist issue's `body`.
    pub fn save_issue(&self, repo: String, token: String, number: u64, body: String) {
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("native mods issue".into())
            .spawn(move || {
                let url = format!("https://api.github.com/repos/{repo}/issues/{number}");
                let payload = serde_json::json!({ "body": body }).to_string();
                let result = github(ureq::patch(&url), &token)
                    .set("Content-Type", "application/json")
                    .send_string(&payload)
                    .map(|_| ())
                    .map_err(github_error);
                let _ = tx.send(Message::IssueSaved(result));
            })
            .ok();
    }

    /// What arrived since the last poll.
    pub fn poll(&self) -> Vec<Message> {
        self.rx.try_iter().collect()
    }
}

/// Download `id`'s newest version for this game, check it and move it into
/// its folder; a mod is also turned on in `horizon-mods.txt`.
fn install(
    index: &Index,
    id: &str,
    game_dir: &Path,
    progress: impl Fn(u64, u64),
) -> Result<String, String> {
    let project = index
        .project(id)
        .ok_or_else(|| format!("{id} is not in the registry"))?;
    let version = match offer(index, project, game_dir) {
        Offer::Get(version) => version,
        Offer::Installed => return Ok(String::new()),
        Offer::PcOnly | Offer::Unavailable => {
            return Err(format!("{} can't be installed here", project.name));
        }
    };
    let folder = folder(&project.kind).expect("an offered kind has a folder");
    if !safe_name(&version.file.name) {
        return Err(format!("{:?} is not a plain file name", version.file.name));
    }
    let total = version.file.size.min(MAX_FILE_BYTES);
    let bytes = get(&version.file.url, total, &progress)?;
    let downloads = game_dir.join(".horizon-downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let partial = downloads.join(format!("{}.part", version.file.sha256));
    std::fs::write(&partial, &bytes).map_err(|e| e.to_string())?;
    if let Err(e) = check_file(index, project, version, &partial) {
        let _ = std::fs::remove_file(&partial);
        return Err(e);
    }
    let dir = game_dir.join(folder);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let to = dir.join(&version.file.name);
    std::fs::rename(&partial, &to).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        e.to_string()
    })?;
    if project.kind == Kind::Mod {
        // Off until turned on, like any installed mod: a mod got from the
        // browser is wanted on.
        let mut list = crate::mod_list::ModList::load(game_dir);
        list.enabled.insert(project.id.clone());
        list.save(game_dir);
    }
    log::info!(
        "Horizon: installed {} {} into {}",
        project.id,
        version.version,
        dir.display()
    );
    Ok(version.file.name.clone())
}

fn github(request: ureq::Request, token: &str) -> ureq::Request {
    request
        .set("User-Agent", AGENT)
        .set("Accept", "application/vnd.github+json")
        .set("Authorization", &format!("Bearer {token}"))
}

fn github_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(401, _) => "GitHub refused the token (401): paste a new one".into(),
        ureq::Error::Status(403, _) | ureq::Error::Status(404, _) => {
            "the token can't reach the repository's issues: give it Issues read and write".into()
        }
        e => e.to_string(),
    }
}

/// The open checklist issue of `repo`.
fn find_issue(repo: &str, token: &str) -> Result<(u64, String), String> {
    let url = format!("https://api.github.com/repos/{repo}/issues?state=open&per_page=100");
    let text = github(ureq::get(&url), token)
        .call()
        .map_err(github_error)?
        .into_string()
        .map_err(|e| e.to_string())?;
    let issues: Vec<serde_json::Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    issues
        .iter()
        .find(|i| i["title"] == ISSUE_TITLE && i.get("pull_request").is_none())
        .and_then(|i| {
            Some((
                i["number"].as_u64()?,
                i["body"].as_str().unwrap_or("").to_owned(),
            ))
        })
        .ok_or_else(|| {
            format!(
                "no \"{ISSUE_TITLE}\" issue: run Actions > Native mods for iOS (checklist) once"
            )
        })
}

/// A file name with no path in it.
fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', '\0']) && name != ".."
}

/// `url`'s body, at most `max` bytes, over HTTPS only.
fn get(url: &str, max: u64, progress: impl Fn(u64, u64)) -> Result<Vec<u8>, String> {
    use std::io::Read;
    if !url.starts_with("https://") {
        return Err(format!("{url} is not an https address"));
    }
    let response = ureq::get(url)
        .set("User-Agent", AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    let mut reader = response.into_reader().take(max + 1);
    let mut body = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        progress(body.len() as u64, max);
    }
    if body.len() as u64 > max {
        return Err(format!("{url} is larger than {max} bytes"));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> Index {
        let version = |name: &str, sha: &str| {
            format!(
                r#"{{"version":"1.0.0","published":"2026-11-01","api":"0.2","game":"26.3",
                "file":{{"name":"{name}","url":"https://github.com/horizon-rust/horizon-mods/releases/download/x/{name}","size":10,"sha256":"{sha}"}}}}"#
            )
        };
        let project = |id: &str, kind: &str, downloads: u64, file: &str| {
            format!(
                r#"{{"id":"{id}","kind":"{kind}","name":"{id}","summary":"s","description":"d","authors":["a"],
                "owners":["o"],"license":"MIT","source":"https://github.com/a/b","categories":["utility"],
                "downloads":{downloads},"versions":[{}]}}"#,
                version(file, &"ab".repeat(32))
            )
        };
        let text = format!(
            r#"{{"schema":1,"projects":[{},{},{},{},{}],"revoked":[]}}"#,
            project("coords", "mod", 5, "coords-1.0.0.hmod"),
            project("fast", "native-mod", 50, "fast-1.0.0.hnmod"),
            project("faithful", "resourcepack", 9, "faithful.zip"),
            project("glow", "shader", 1, "glow.hshader"),
            project("popular", "mod", 99, "popular-1.0.0.hmod"),
        );
        Index::parse_local(&text).expect("the test index parses")
    }

    #[test]
    fn each_tab_lists_its_kinds_most_downloaded_first() {
        let index = index();
        let ids = |tab| {
            entries(&index, tab)
                .iter()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(Tab::Mods), ["popular", "coords"]);
        assert_eq!(ids(Tab::Native), ["fast"]);
        assert_eq!(ids(Tab::ResourcePacks), ["faithful"]);
        assert_eq!(ids(Tab::Shaders), ["glow"]);
    }

    #[test]
    fn native_mods_are_pc_only_and_installed_files_are_marked() {
        let index = index();
        let dir = std::env::temp_dir().join(format!("horizon-mod-registry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fast = index.project("fast").unwrap();
        assert_eq!(offer(&index, fast, &dir), Offer::PcOnly);
        let coords = index.project("coords").unwrap();
        assert!(
            matches!(offer(&index, coords, &dir), Offer::Get(v) if v.file.name == "coords-1.0.0.hmod")
        );
        std::fs::create_dir_all(dir.join("horizon-mods")).unwrap();
        std::fs::write(dir.join("horizon-mods/coords-1.0.0.hmod"), b"x").unwrap();
        assert_eq!(offer(&index, coords, &dir), Offer::Installed);
        let faithful = index.project("faithful").unwrap();
        std::fs::create_dir_all(dir.join("resourcepacks")).unwrap();
        std::fs::write(dir.join("resourcepacks/faithful.zip"), b"x").unwrap();
        assert_eq!(offer(&index, faithful, &dir), Offer::Installed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_plain_file_names_are_written() {
        assert!(safe_name("coords-1.0.0.hmod"));
        for bad in ["", "../x.zip", "a/b.zip", ".hidden", "..", "a\\b"] {
            assert!(!safe_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn my_mods_are_the_newest_published_release_of_each() {
        let release = |tag: &str, created: &str, meta: &str, asset: &str| {
            serde_json::json!({
                "tag_name": tag,
                "created_at": created,
                "body": format!("A mod.\n\n<!-- horizon-user-mod {meta} -->"),
                "assets": [{"name": asset, "browser_download_url": format!("https://github.com/o/r/releases/download/{tag}/{asset}")}],
            })
        };
        let list = serde_json::Value::Array(vec![
            release(
                "mod-gems-v0.1.0",
                "2026-11-06T10:00:00Z",
                r#"{"id": "gems", "kind": "content", "version": "0.1.0", "sha256": "AB", "size": 5}"#,
                "gems.hmod",
            ),
            release(
                "mod-gems-v0.2.0",
                "2026-11-07T10:00:00Z",
                r#"{"id": "gems", "kind": "content", "version": "0.2.0", "sha256": "CD", "size": 6}"#,
                "gems.hmod",
            ),
            release(
                "mod-hud-v1",
                "2026-11-07T11:00:00Z",
                r#"{"id": "hud", "kind": "native", "version": "1", "sha256": "ef", "size": 7}"#,
                "hud.hnmod",
            ),
            // Not the mod maker's: no mark, or another file than its id's.
            serde_json::json!({"tag_name": "ios-r22", "body": "Horizon", "assets": []}),
            release(
                "mod-odd-v1",
                "2026-11-07T11:00:00Z",
                r#"{"id": "odd", "kind": "content", "version": "1", "sha256": "00", "size": 1}"#,
                "other.hmod",
            ),
        ]);
        let mine = user_mods(&list.to_string());
        assert_eq!(mine.len(), 2);
        let gems = &mine[0];
        assert_eq!(
            (gems.id.as_str(), gems.version.as_str(), gems.native),
            ("gems", "0.2.0", false)
        );
        assert_eq!(gems.sha256, "cd");
        assert!(gems.url.ends_with("/mod-gems-v0.2.0/gems.hmod"));
        assert!(mine[1].native);
        assert!(user_mods("not json").is_empty());
    }

    #[test]
    fn the_checklist_keeps_its_text_and_takes_the_ticks() {
        let body = "Intro\n\n- [ ] **Build now**\n\n- [x] `zoom` Zoom: a\n- [ ] `minimap` Minimap: b `code`\n";
        assert_eq!(ticked(body).into_iter().collect::<Vec<_>>(), ["zoom"]);
        let ticks = ["minimap".to_owned()].into_iter().collect();
        let built = with_ticks(body, &ticks, true);
        assert_eq!(
            built,
            "Intro\n\n- [x] **Build now**\n\n- [ ] `zoom` Zoom: a\n- [x] `minimap` Minimap: b `code`\n"
        );
        assert_eq!(ticked(&built).into_iter().collect::<Vec<_>>(), ["minimap"]);
        assert!(with_ticks(&built, &ticks, false).contains("- [ ] **Build now**"));
    }
}
