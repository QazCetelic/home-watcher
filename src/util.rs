use microxdg::Xdg;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use uzers::get_current_username;
use uzers::get_user_by_name;
use uzers::os::unix::UserExt;

// TODO Documents is sometimes abused to store program files, create an option to watch it regardless
pub const DEFAULT_DIRECTORIES: [&str; 7] = ["Documents", "Downloads", "Pictures", "Videos", "Desktop", "Music", ".var"];

/// The user who started the session; the effective user if there is none (e.g. systemd).
pub fn get_user() -> String {
    match Command::new("logname").stderr(Stdio::null()).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => get_current_username()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "root".to_string()),
    }
}

const HOME_WATCHER_DB_FILE: &str = "home-watcher.db";

/// Resolves the XDG base directories for `user`.
pub fn get_xdg_dirs(user: &str) -> Xdg {
    let user = get_user_by_name(user).expect("unknown user");
    Xdg::with_home(user.home_dir())
}

fn data_home(xdg_dirs: &Xdg) -> PathBuf {
    xdg_dirs.data().unwrap_or_else(|_| xdg_dirs.home().join(".local/share"))
}

/// The user-specific XDG base directories, all of which are excluded from monitoring.
fn user_directories(xdg_dirs: &Xdg) -> Vec<PathBuf> {
    vec![
        xdg_dirs.config().unwrap_or_else(|_| xdg_dirs.home().join(".config")),
        xdg_dirs.cache().unwrap_or_else(|_| xdg_dirs.home().join(".cache")),
        data_home(xdg_dirs),
        xdg_dirs.state().unwrap_or_else(|_| xdg_dirs.home().join(".local/state")),
    ]
}

pub fn get_db_path(xdg_dirs: &Xdg) -> PathBuf {
    data_home(xdg_dirs).join(HOME_WATCHER_DB_FILE)
}

pub fn get_excluded_directories(
    xdg_dirs: &Xdg,
    args_excluded_dirs: &Option<Vec<String>>,
    args_include_dirs: &Option<Vec<String>>,
) -> Vec<String> {
    // Absolute paths to keep watched even though they are excluded by default.
    let mut reincluded: BTreeSet<String> = BTreeSet::new();
    if let Some(dirs) = args_include_dirs {
        for dir in dirs {
            reincluded.insert(format!("{}/{}", xdg_dirs.home().display(), dir));
        }
    }

    let mut excluded_dirs: BTreeSet<String> = BTreeSet::new();
    for dir in args_excluded_dirs.iter().flatten().map(String::as_str).chain(DEFAULT_DIRECTORIES) {
        excluded_dirs.insert(format!("{}/{}", xdg_dirs.home().display(), dir));
    }
    for dir in user_directories(xdg_dirs) {
        excluded_dirs.insert(dir.to_string_lossy().into_owned());
    }

    excluded_dirs.retain(|dir| !reincluded.contains(dir));
    excluded_dirs.into_iter().collect()
}