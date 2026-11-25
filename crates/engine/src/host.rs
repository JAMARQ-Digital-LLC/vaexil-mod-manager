//! What the engine asks of Windows: where the game is (Steam, Epic, the
//! Microsoft Store), the app's own data folder, which processes are running,
//! and handing a link or file to Windows to open. Nothing here writes,
//! except making the app's own data folder private to this user.

use std::path::PathBuf;

use vmm_core::error::{ErrorCode, VmmError};
use vmm_core::game::{GameInstall, Sources, Store};

/// `VAEXIL_MOD_MANAGER_GAME` names a game folder to use instead of looking
/// for installs. Development and smoke-test runs use it to stay away from
/// the real game.
pub fn game_override() -> Option<GameInstall> {
    let dir = std::env::var_os("VAEXIL_MOD_MANAGER_GAME").filter(|d| !d.is_empty())?;
    Some(GameInstall { store: Store::Folder, root: PathBuf::from(dir) })
        .filter(|g| !crate::scratch::active() || game_allowed(&g.root).is_ok())
}

/// In scratch, refuses a game folder inside the real places
/// ([`crate::scratch::check_path`]).
pub fn game_allowed(root: &std::path::Path) -> Result<(), VmmError> {
    if crate::scratch::active() {
        crate::scratch::check_path("The game folder", root, &crate::scratch::Real::from_env())?;
    }
    Ok(())
}

/// The app's own folder: `%LOCALAPPDATA%\Vaexil Mod Manager` for the
/// release build, `%LOCALAPPDATA%\Vaexil Mod Manager Dev` for a dev build.
/// `VAEXIL_MOD_MANAGER_DATA_DIR` (or the old `VAEXIL_MOD_MANAGER_DATA`)
/// replaces it, so tests stay out of the user's profile. See [`crate::scratch`].
pub fn data_dir() -> Option<PathBuf> {
    crate::scratch::data_dir()
}

/// Creates `dir` if needed and makes it private to this user: on Windows a
/// protected access list that grants only the signed-in user full control,
/// so neither inherited entries nor other accounts reach it; elsewhere 0700.
/// Does nothing when it already is.
pub fn prepare_data_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    make_private(dir)
}

#[cfg(windows)]
fn make_private(dir: &std::path::Path) -> std::io::Result<()> {
    let want = private_sddl()?;
    if read_dacl(dir).ok().as_deref() == Some(want.as_str()) {
        return Ok(());
    }
    acl::set_dacl(dir, &want)
}

#[cfg(not(windows))]
fn make_private(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// The access list a private data folder gets, as SDDL: protected (`P`),
/// one entry granting the current user full control, inherited by files
/// and folders inside (`OICI`).
#[cfg(windows)]
pub fn private_sddl() -> std::io::Result<String> {
    Ok(format!("D:P(A;OICI;FA;;;{})", acl::current_user_sid()?))
}

/// `dir`'s access list as SDDL (`D:...`), without the `AI` flag Windows
/// adds once a list has been set through it (auto-inherit), which doesn't
/// change who has access.
#[cfg(windows)]
pub fn read_dacl(dir: &std::path::Path) -> std::io::Result<String> {
    let sddl = acl::get_dacl(dir)?;
    let split = sddl.find('(').unwrap_or(sddl.len());
    let (flags, entries) = sddl.split_at(split);
    Ok(format!("{}{entries}", flags.replace("AI", "")))
}

#[cfg(windows)]
mod acl {
    use std::io;
    use std::path::Path;

    use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
        ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
        SetNamedSecurityInfoW,
    };
    use windows::Win32::Security::{
        ACL, DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl, GetTokenInformation,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::core::{BOOL, HSTRING, PWSTR};

    fn os(e: windows::core::Error) -> io::Error {
        io::Error::other(e.to_string())
    }

    pub fn current_user_sid() -> io::Result<String> {
        // SAFETY: the token is opened for query and closed; the buffer is
        // sized by the first call; the string is freed with LocalFree.
        unsafe {
            let mut token = HANDLE::default();
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).map_err(os)?;
            let mut len = 0u32;
            let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
            let mut buf = vec![0u8; len as usize];
            let r = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr().cast()), len, &mut len);
            let _ = CloseHandle(token);
            r.map_err(os)?;
            let user = &*(buf.as_ptr() as *const TOKEN_USER);
            let mut s = PWSTR::null();
            ConvertSidToStringSidW(user.User.Sid, &mut s).map_err(os)?;
            let out = s.to_string().map_err(|e| io::Error::other(e.to_string()));
            let _ = LocalFree(Some(HLOCAL(s.0.cast())));
            out
        }
    }

    pub fn set_dacl(dir: &Path, sddl: &str) -> io::Result<()> {
        // SAFETY: the descriptor comes from Windows and is freed with
        // LocalFree after the call that reads its access list.
        unsafe {
            let mut sd = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(&HSTRING::from(sddl), SDDL_REVISION_1, &mut sd, None)
                .map_err(os)?;
            let mut present = BOOL(0);
            let mut defaulted = BOOL(0);
            let mut dacl: *mut ACL = std::ptr::null_mut();
            let r = GetSecurityDescriptorDacl(sd, &mut present, &mut dacl, &mut defaulted).map_err(os).and_then(|()| {
                SetNamedSecurityInfoW(
                    &HSTRING::from(dir.as_os_str()),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    Some(dacl),
                    None,
                )
                .ok()
                .map_err(os)
            });
            let _ = LocalFree(Some(HLOCAL(sd.0)));
            r
        }
    }

    /// Whether `sid` (SDDL: `S-1-...` or an alias such as `LA`) is the
    /// current user's.
    #[cfg(test)]
    pub fn is_current_user(sid: &str) -> io::Result<bool> {
        use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
        use windows::Win32::Security::{EqualSid, PSID};
        let mine = current_user_sid()?;
        // SAFETY: both SIDs come from Windows and are freed with LocalFree.
        unsafe {
            let (mut a, mut b) = (PSID::default(), PSID::default());
            ConvertStringSidToSidW(&HSTRING::from(sid), &mut a).map_err(os)?;
            let r = ConvertStringSidToSidW(&HSTRING::from(mine), &mut b).map_err(os);
            let same = r.map(|()| EqualSid(a, b).is_ok());
            let _ = LocalFree(Some(HLOCAL(a.0)));
            if !b.0.is_null() {
                let _ = LocalFree(Some(HLOCAL(b.0)));
            }
            same
        }
    }

    pub fn get_dacl(dir: &Path) -> io::Result<String> {
        // SAFETY: Windows allocates the descriptor and the string; both are
        // freed with LocalFree.
        unsafe {
            let mut sd = PSECURITY_DESCRIPTOR::default();
            GetNamedSecurityInfoW(
                &HSTRING::from(dir.as_os_str()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                &mut sd,
            )
            .ok()
            .map_err(os)?;
            let mut s = PWSTR::null();
            let r = ConvertSecurityDescriptorToStringSecurityDescriptorW(
                sd,
                SDDL_REVISION_1,
                DACL_SECURITY_INFORMATION,
                &mut s,
                None,
            )
            .map_err(os)
            .and_then(|()| s.to_string().map_err(|e| io::Error::other(e.to_string())));
            let _ = LocalFree(Some(HLOCAL(s.0.cast())));
            let _ = LocalFree(Some(HLOCAL(sd.0)));
            r
        }
    }
}

/// Where the real Steam, Epic and Microsoft Store installs are. Empty in a
/// dev build and in scratch ([`crate::scratch::real_host_allowed`]).
pub fn sources() -> Sources {
    if !crate::scratch::real_host_allowed() {
        return Sources { steam: None, epic_manifests: None, xbox_roots: Vec::new() };
    }
    Sources { steam: steam_root(), epic_manifests: epic_manifests(), xbox_roots: xbox_roots() }
}

fn steam_root() -> Option<PathBuf> {
    registry_steam_path().map(PathBuf::from).or_else(|| {
        let default = PathBuf::from(r"C:\Program Files (x86)\Steam");
        default.is_dir().then_some(default)
    })
}

#[cfg(windows)]
pub(crate) fn registry_steam_path() -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::w;

    let mut buf = [0u16; 1024];
    let mut len = (buf.len() * 2) as u32;
    // SAFETY: the buffer and its length in bytes are valid for the call.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Valve\Steam"),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if status.is_err() {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let path = String::from_utf16_lossy(&buf[..end]);
    (!path.is_empty()).then_some(path)
}

#[cfg(not(windows))]
pub(crate) fn registry_steam_path() -> Option<String> {
    None
}

fn epic_manifests() -> Option<PathBuf> {
    let base = std::env::var_os("ProgramData").map(PathBuf::from)?;
    let dir = base.join(r"Epic\EpicGamesLauncher\Data\Manifests");
    dir.is_dir().then_some(dir)
}

/// `X:\XboxGames` on every drive that has one, where the Microsoft Store
/// installs games by default.
fn xbox_roots() -> Vec<PathBuf> {
    (b'A'..=b'Z').map(|d| PathBuf::from(format!("{}:\\XboxGames", d as char))).filter(|p| p.is_dir()).collect()
}

/// Which of `names` (executable file names, any case) are running now.
#[cfg(windows)]
pub fn running(names: &[&str]) -> Vec<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };

    let mut found = Vec::new();
    // SAFETY: a process snapshot, read with a correctly sized entry and closed.
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return found };
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let end = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            let exe = String::from_utf16_lossy(&entry.szExeFile[..end]);
            if names.iter().any(|n| n.eq_ignore_ascii_case(&exe)) && !found.contains(&exe) {
                found.push(exe);
            }
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    found
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn the_data_folder_is_private_to_this_user() {
        // One entry: allow, inherited by files and folders (OICI), full
        // access, for this user. Windows may write the SID as an alias
        // (LA for the built-in Administrator, as on CI), so the SIDs are
        // compared, not the text.
        fn private_to_me(sddl: &str) -> bool {
            let Some(rest) = sddl.strip_prefix("D:P(A;OICI;FA;;;") else { return false };
            let Some(sid) = rest.strip_suffix(')') else { return false };
            !sid.contains('(') && super::acl::is_current_user(sid).unwrap()
        }
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("data");
        super::prepare_data_dir(&dir).unwrap();
        assert!(private_to_me(&super::private_sddl().unwrap()));
        let got = super::read_dacl(&dir).unwrap();
        assert!(private_to_me(&got), "{got}");
        // A file inside inherits only that entry.
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        let file = super::read_dacl(&dir.join("settings.json")).unwrap();
        assert_eq!(file.matches('(').count(), 1, "{file}");
        // An existing folder with the usual inherited entries is fixed at launch.
        let other = tmp.path().join("existing");
        std::fs::create_dir(&other).unwrap();
        let before = super::read_dacl(&other).unwrap();
        assert!(!private_to_me(&before), "{before}");
        super::prepare_data_dir(&other).unwrap();
        let after = super::read_dacl(&other).unwrap();
        assert!(private_to_me(&after), "{after}");
    }

    #[test]
    fn run_command_waits_and_returns_the_exit_code() {
        let cmd = std::env::var("ComSpec").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into());
        let code = super::run_command(&[cmd, "/c".into(), "exit".into(), "3".into()]).unwrap();
        assert_eq!(code, 3);
        assert!(super::run_command(&[]).is_err());
    }
}

#[cfg(not(windows))]
pub fn running(_names: &[&str]) -> Vec<String> {
    Vec::new()
}

/// Runs the game's own command, as Steam passes it in `%command%`, from its
/// folder, and waits for it to end, so Steam sees the game running for as
/// long as it does. Returns its exit code.
pub fn run_command(command: &[String]) -> Result<i32, VmmError> {
    let program = command.first().ok_or_else(|| {
        VmmError::new(ErrorCode::LaunchUnsupported, "Steam didn't pass the game's command (%command%).")
    })?;
    let program = PathBuf::from(program);
    let cwd = program.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let mut child = std::process::Command::new(&program)
        .args(&command[1..])
        .current_dir(&cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            VmmError::new(ErrorCode::LaunchUnsupported, format!("Couldn't start {}: {e}", program.display()))
        })?;
    let status = child.wait().map_err(|e| VmmError::new(ErrorCode::Io, format!("Lost track of the game: {e}")))?;
    Ok(status.code().unwrap_or(0))
}

/// Opens a link or a file with what Windows has registered for it, such as
/// `steam://rungameid/...` with Steam, or a log file with the text editor.
#[cfg(windows)]
pub fn open(target: &str) -> Result<(), VmmError> {
    if crate::scratch::active() && target.contains("://") {
        return Err(VmmError::new(
            ErrorCode::ScratchRefused,
            format!("Scratch runs don't open links such as {target}, which could start the real game."),
        ));
    }
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, w};

    let target_w = HSTRING::from(target);
    // SAFETY: plain shell call with valid strings.
    let result = unsafe { ShellExecuteW(None, w!("open"), &target_w, None, None, SW_SHOWNORMAL) };
    // ShellExecute reports success with a value above 32.
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err(VmmError::new(ErrorCode::LaunchUnsupported, format!("Windows could not open {target}.")))
    }
}

#[cfg(not(windows))]
pub fn open(target: &str) -> Result<(), VmmError> {
    Err(VmmError::new(ErrorCode::LaunchUnsupported, format!("Opening {target} is only supported on Windows.")))
}
