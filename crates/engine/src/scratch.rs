//! Which copy of the app this is, and the scratch mode that keeps tests and
//! development runs away from the real data, Steam, AI apps and network
//! (agentic-instructions `docs/desktop-app-standard.md`, section 5).
//!
//! - A **release build** is a release-profile build made by GitHub Actions
//!   or from the shared checkout (`C:\JAMARQ\Apps\vaexil-mod-manager`), as
//!   `build.rs` decides. It keeps `%LOCALAPPDATA%\Vaexil Mod Manager` and
//!   finds the real game, Steam and AI apps.
//! - Every other build (a debug build, any build from a worktree) is a
//!   **dev build**: its folder is `%LOCALAPPDATA%\Vaexil Mod Manager Dev`, it
//!   never looks for the real game, Steam or AI apps (only the folders the
//!   variables below name), and it never updates itself.
//! - **Scratch** (`VAEXIL_MOD_MANAGER_SCRATCH=1`, and always in this crate's
//!   own tests) needs every folder named and refuses, rather than warns,
//!   when one is under the app's real folders, `%APPDATA%` or the real
//!   Steam, and it makes no outside connections.
//!
//! The variables: `_DATA_DIR` (`_DATA` is the old name and still works),
//! `_GAME`, `_STEAM`, `_AI_APPS`, `_NETWORK` (`live` or `offline`) and
//! `_SCRATCH`, each after `VAEXIL_MOD_MANAGER`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use vmm_core::error::{ErrorCode, VmmError};

pub const PREFIX: &str = "VAEXIL_MOD_MANAGER_";
/// The release build's data folder, under `%LOCALAPPDATA%`.
pub const RELEASE_FOLDER: &str = "Vaexil Mod Manager";
/// A dev build's data folder, under `%LOCALAPPDATA%`.
pub const DEV_FOLDER: &str = "Vaexil Mod Manager Dev";

/// The folders scratch mode needs named.
const REQUIRED: [&str; 4] = ["DATA_DIR", "GAME", "STEAM", "AI_APPS"];

fn var(name: &str) -> Option<OsString> {
    std::env::var_os(format!("{PREFIX}{name}")).filter(|v| !v.is_empty())
}

/// Whether this is the release build (see the module notes).
pub fn release_build() -> bool {
    option_env!("VMM_RELEASE_IDENTITY").is_some()
}

/// Whether scratch mode is on.
pub fn active() -> bool {
    cfg!(test) || var("SCRATCH").is_some_and(|v| v == "1")
}

/// Whether the real game, Steam and AI apps may be looked for.
pub fn real_host_allowed() -> bool {
    release_build() && !active()
}

/// Whether outside connections are refused: in scratch, or with
/// `VAEXIL_MOD_MANAGER_NETWORK=offline`.
pub fn offline() -> bool {
    active() || var("NETWORK").is_some_and(|v| v == "offline")
}

fn refused(message: impl Into<String>) -> VmmError {
    VmmError::new(ErrorCode::ScratchRefused, message)
}

/// Refuses an outside connection to `url` when [`offline`].
pub fn check_network(url: &str) -> Result<(), VmmError> {
    if offline() {
        return Err(refused(format!(
            "This run makes no outside connections (scratch or {PREFIX}NETWORK=offline), so {url} wasn't fetched."
        )));
    }
    Ok(())
}

/// Refuses checking for and installing Vaexil Mod Manager's own updates,
/// except in the release build outside scratch: an update installs the
/// release over the user's real copy.
pub fn check_self_update() -> Result<(), VmmError> {
    if !real_host_allowed() {
        return Err(refused(
            "This is a development or scratch build of Vaexil Mod Manager, which never checks for or installs \
             updates. The installed app updates itself.",
        ));
    }
    Ok(())
}

/// Refuses changes to the user's real settings outside the app (the PATH)
/// from a dev or scratch build.
pub fn check_real_settings(what: &str) -> Result<(), VmmError> {
    if !real_host_allowed() {
        return Err(refused(format!(
            "This is a development or scratch build of Vaexil Mod Manager, so it doesn't change {what}. The \
             installed app does."
        )));
    }
    Ok(())
}

/// The data folder: `_DATA_DIR` (or the old `_DATA`), else the build's own
/// folder under `%LOCALAPPDATA%`. `None` in scratch without a named folder
/// or with one [`check_path`] refuses, and when `%LOCALAPPDATA%` isn't set;
/// then nothing that needs a backup is allowed.
pub fn data_dir() -> Option<PathBuf> {
    if let Some(dir) = named_data_dir() {
        let dir = PathBuf::from(dir);
        if active() && check_path("The data folder", &dir, &Real::from_env()).is_err() {
            return None;
        }
        return Some(dir);
    }
    if active() {
        return None;
    }
    let base = std::env::var_os("LOCALAPPDATA").filter(|d| !d.is_empty())?;
    Some(PathBuf::from(base).join(if release_build() { RELEASE_FOLDER } else { DEV_FOLDER }))
}

fn named_data_dir() -> Option<OsString> {
    var("DATA_DIR").or_else(|| var("DATA"))
}

/// The real places scratch keeps away from, read from this PC.
#[derive(Debug, Clone, Default)]
pub struct Real {
    pub local_appdata: Option<PathBuf>,
    pub appdata: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub steam: Vec<PathBuf>,
    pub path: Vec<PathBuf>,
}

impl Real {
    pub fn from_env() -> Real {
        let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let mut steam = vec![PathBuf::from(r"C:\Program Files (x86)\Steam")];
        steam.extend(crate::host::registry_steam_path().map(PathBuf::from));
        Real {
            local_appdata: env("LOCALAPPDATA"),
            appdata: env("APPDATA"),
            home: env("USERPROFILE"),
            steam,
            path: std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default(),
        }
    }

    /// Folders nothing in scratch may be inside.
    fn forbidden(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(l) = &self.local_appdata {
            out.push(l.join(RELEASE_FOLDER));
            out.push(l.join(DEV_FOLDER));
            out.push(l.join("Programs").join(RELEASE_FOLDER));
            out.push(l.join("AnthropicClaude"));
        }
        out.extend(self.appdata.clone());
        if let Some(h) = &self.home {
            for d in [".local", ".claude", ".gemini", ".codex"] {
                out.push(h.join(d));
            }
        }
        out.extend(self.steam.iter().cloned());
        out
    }
}

/// Lower case, backslashes, no trailing separator, no `\\?\`, `.` and `..`
/// resolved, and for the part that exists, short (8.3) names expanded.
fn normalize(p: &Path) -> String {
    let abs = std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    // Canonicalize the longest part that exists, then add back the rest.
    let mut existing = abs.as_path();
    let mut rest = Vec::new();
    let resolved = loop {
        if let Ok(c) = std::fs::canonicalize(existing) {
            break rest.iter().rev().fold(c, |acc: PathBuf, part| acc.join(part));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                existing = parent;
            }
            _ => break abs.clone(),
        }
    };
    let s = resolved.to_string_lossy().replace('/', "\\");
    let s = s.strip_prefix(r"\\?\UNC\").map(|u| format!(r"\\{u}")).unwrap_or(s);
    let s = s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s);
    s.trim_end_matches('\\').to_lowercase()
}

/// Whether `p` is `root` or inside it.
fn inside(p: &str, root: &str) -> bool {
    !root.is_empty() && (p == root || p.strip_prefix(root).is_some_and(|r| r.starts_with('\\')))
}

/// Refuses `path` (`what` names it) when it is inside one of the real
/// places, or, for the AI apps' folder, a folder on the real PATH.
pub fn check_path(what: &str, path: &Path, real: &Real) -> Result<(), VmmError> {
    let p = normalize(path);
    for root in real.forbidden() {
        if inside(&p, &normalize(&root)) {
            return Err(refused(format!(
                "{what} ({}) is inside {}, which scratch runs never use. Name a throwaway folder instead.",
                path.display(),
                root.display()
            )));
        }
    }
    Ok(())
}

/// Scratch's checks, against `var` (a variable's value, without the
/// prefix) and `real`: every folder named and none of them real, and the
/// network not `live`. Also `NETWORK` must be `live` or `offline`.
pub fn check_with(var: impl Fn(&str) -> Option<OsString>, scratch: bool, real: &Real) -> Result<(), VmmError> {
    if let Some(n) = var("NETWORK")
        && n != "live"
        && n != "offline"
    {
        return Err(refused(format!("{PREFIX}NETWORK is {}; use live or offline.", n.to_string_lossy())));
    }
    if !scratch {
        return Ok(());
    }
    if var("NETWORK").is_some_and(|n| n == "live") {
        return Err(refused(format!(
            "Scratch runs make no outside connections; unset {PREFIX}NETWORK or make it offline."
        )));
    }
    let missing: Vec<String> = REQUIRED
        .iter()
        .filter(
            |k| if **k == "DATA_DIR" { var("DATA_DIR").or_else(|| var("DATA")).is_none() } else { var(k).is_none() },
        )
        .map(|k| format!("{PREFIX}{k}"))
        .collect();
    if !missing.is_empty() {
        return Err(refused(format!(
            "Scratch runs need a throwaway folder for each of {}, so nothing real is used.",
            missing.join(", ")
        )));
    }
    for (k, what) in [
        ("DATA_DIR", "The data folder"),
        ("DATA", "The data folder"),
        ("GAME", "The game folder"),
        ("STEAM", "The Steam folder"),
        ("AI_APPS", "The AI apps folder"),
    ] {
        if let Some(v) = var(k) {
            check_path(what, Path::new(&v), real)?;
        }
    }
    if let Some(ai) = var("AI_APPS") {
        let ai = normalize(Path::new(&ai));
        if real.path.iter().any(|d| normalize(d) == ai) {
            return Err(refused("The AI apps folder is on the real PATH; name a throwaway folder instead."));
        }
    }
    Ok(())
}

/// What every front end runs first: the checks above for this process, and
/// in scratch, the data folder created private to this user. An error
/// means the run must stop.
pub fn check() -> Result<(), VmmError> {
    check_with(var, active(), &Real::from_env())?;
    if active() {
        let dir = data_dir().ok_or_else(|| refused("Scratch runs need a throwaway data folder."))?;
        crate::host::prepare_data_dir(&dir)
            .map_err(|e| refused(format!("Couldn't make the data folder {} private: {e}", dir.display())))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn real(tmp: &Path) -> Real {
        Real {
            local_appdata: Some(tmp.join("Local")),
            appdata: Some(tmp.join("Roaming")),
            home: Some(tmp.join("home")),
            steam: vec![tmp.join("Steam")],
            path: vec![tmp.join("bin")],
        }
    }

    fn vars(pairs: &[(&str, PathBuf)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone().into())).collect();
        move |k| map.get(k).cloned()
    }

    fn all(tmp: &Path) -> Vec<(&'static str, PathBuf)> {
        let s = tmp.join("scratch");
        vec![
            ("DATA_DIR", s.join("data")),
            ("GAME", s.join("game")),
            ("STEAM", s.join("steam")),
            ("AI_APPS", s.join("ai")),
        ]
    }

    #[test]
    fn this_crates_tests_run_in_scratch() {
        assert!(active());
        assert!(offline());
        assert!(!real_host_allowed());
        assert_eq!(check_network("https://github.com/").unwrap_err().code, ErrorCode::ScratchRefused);
        assert_eq!(check_self_update().unwrap_err().code, ErrorCode::ScratchRefused);
    }

    #[test]
    fn scratch_needs_every_folder_named() {
        let tmp = tempfile::tempdir().unwrap();
        let real = real(tmp.path());
        assert!(check_with(vars(&all(tmp.path())), true, &real).is_ok());
        for skip in REQUIRED {
            let some: Vec<_> = all(tmp.path()).into_iter().filter(|(k, _)| *k != skip).collect();
            let err = check_with(vars(&some), true, &real).unwrap_err();
            assert_eq!(err.code, ErrorCode::ScratchRefused, "{skip}");
            assert!(err.message.contains(&format!("{PREFIX}{skip}")), "{}", err.message);
        }
        // Outside scratch nothing is required.
        assert!(check_with(vars(&[]), false, &real).is_ok());
    }

    #[test]
    fn the_old_data_name_still_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let mut v = all(tmp.path());
        v[0].0 = "DATA";
        assert!(check_with(vars(&v), true, &real(tmp.path())).is_ok());
    }

    #[test]
    fn scratch_refuses_the_real_places() {
        let tmp = tempfile::tempdir().unwrap();
        let r = real(tmp.path());
        let local = tmp.path().join("Local");
        for (key, bad) in [
            ("DATA_DIR", local.join(RELEASE_FOLDER)),
            ("DATA", local.join(RELEASE_FOLDER).join("sub")),
            ("DATA_DIR", local.join(DEV_FOLDER)),
            ("DATA_DIR", tmp.path().join("Roaming").join("x")),
            ("GAME", tmp.path().join("Steam").join("steamapps").join("common").join("HITMAN 3")),
            ("STEAM", tmp.path().join("Steam")),
            ("AI_APPS", tmp.path().join("Roaming").join("npm")),
            ("AI_APPS", tmp.path().join("home").join(".local").join("bin")),
            ("AI_APPS", tmp.path().join("bin")),
        ] {
            let mut v: Vec<_> = all(tmp.path()).into_iter().filter(|(k, _)| *k != key).collect();
            if key == "DATA" {
                v.retain(|(k, _)| *k != "DATA_DIR");
            }
            v.push((key, bad.clone()));
            let err = check_with(vars(&v), true, &r).unwrap_err();
            assert_eq!(err.code, ErrorCode::ScratchRefused, "{key} {}", bad.display());
        }
        // Another spelling of the same folder is caught too.
        let mut v: Vec<_> = all(tmp.path()).into_iter().filter(|(k, _)| *k != "DATA_DIR").collect();
        let odd = PathBuf::from(local.to_string_lossy().to_uppercase()).join("x").join("..").join(RELEASE_FOLDER);
        v.push(("DATA_DIR", odd));
        assert!(check_with(vars(&v), true, &r).is_err());
        // A temp folder under %LOCALAPPDATA% is fine.
        let mut v: Vec<_> = all(tmp.path()).into_iter().filter(|(k, _)| *k != "DATA_DIR").collect();
        v.push(("DATA_DIR", local.join("Temp").join("vmm")));
        assert!(check_with(vars(&v), true, &r).is_ok());
    }

    #[test]
    fn scratch_refuses_the_live_network() {
        let tmp = tempfile::tempdir().unwrap();
        let r = real(tmp.path());
        let mut v = all(tmp.path());
        v.push(("NETWORK", "live".into()));
        assert!(check_with(vars(&v), true, &r).is_err());
        let mut v = all(tmp.path());
        v.push(("NETWORK", "offline".into()));
        assert!(check_with(vars(&v), true, &r).is_ok());
        assert!(check_with(vars(&[("NETWORK", "sometimes".into())]), false, &r).is_err());
    }

    #[test]
    fn scratch_never_reaches_the_real_host() {
        // This crate's tests are scratch, so with no folder named nothing
        // real is found or used.
        assert!(crate::host::sources().steam.is_none());
        assert!(crate::host::sources().xbox_roots.is_empty());
        if var("AI_APPS").is_none() {
            let places = crate::ai_apps::Places::from_env();
            assert!(places.path.is_empty() && places.home.is_none() && places.appdata.is_none());
        }
        if var("DATA_DIR").is_none() && var("DATA").is_none() {
            assert!(data_dir().is_none());
        }
        assert_eq!(crate::http::get("https://github.com/", "text/html").unwrap_err().code, ErrorCode::ScratchRefused);
    }
}
