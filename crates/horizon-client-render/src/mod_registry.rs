//! Getting mods from Horizon's registry (playhorizon.dev/mods) inside the
//! game, for the iOS build, which has no launcher. It reads the registry's
//! `index.json` (docs/api/mod-index.md) and installs what is only files:
//! content mods (`.hmod`), resource packs and shader packs. Native mods are
//! Rust code a launcher compiles into the game; an iPhone can't, so they are
//! listed as PC only.
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
    ResourcePacks,
    Shaders,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Mods, Tab::ResourcePacks, Tab::Shaders];

    /// The translation key of its name.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Mods => "horizon.mods.tab.mods",
            Tab::ResourcePacks => "horizon.mods.tab.resourcepacks",
            Tab::Shaders => "horizon.mods.tab.shaders",
        }
    }

    fn holds(self, kind: &Kind) -> bool {
        match self {
            Tab::Mods => matches!(kind, Kind::Mod | Kind::NativeMod),
            Tab::ResourcePacks => *kind == Kind::ResourcePack,
            Tab::Shaders => matches!(kind, Kind::Shader | Kind::ShaderPack),
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

/// What the worker reports.
#[derive(Debug)]
pub enum Message {
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
        assert_eq!(ids(Tab::Mods), ["popular", "fast", "coords"]);
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
}
