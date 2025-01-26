use crate::standard_dirs::StandardDirectories;
use crate::user_env::UserEnvironment;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::path::PathBuf;
use std::process::Command;

// TODO Documents is sometimes abused to store program files, create an option to watch it regardless
pub const DEFAULT_DIRECTORIES: [&str; 6] = ["Documents", "Downloads", "Pictures", "Videos", "Desktop", "Music"];

pub fn get_user() -> String {
    if let Ok(logname_output) = Command::new("logname").output() {
        String::from_utf8(logname_output.stdout).expect("Failed to get stdout from logname").trim_end().to_string()
    }
    else {
        let who_output = Command::new("who").output().expect("Failed to execute who");
        let mut lines = who_output.stdout.lines();
        let first_line = lines.next().unwrap().unwrap();
        first_line.split_once(" ").unwrap().0.to_string()
    }
}

const HOME_WATCHER_DB_FILE: &str = "home-watcher.db";
pub fn get_db_path(xdg_dirs: &StandardDirectories) -> String {
    let data_home = xdg_dirs.data_home();
    format!("{data_home}/{HOME_WATCHER_DB_FILE}")
}
pub fn get_default_db_path() -> PathBuf {
    let user = get_user();
    let user_env = UserEnvironment::from_user(&user).expect("Failed to get user env");
    let std_dirs = StandardDirectories::new(&user, &user_env);
    PathBuf::from(get_db_path(&std_dirs))
}

pub fn get_excluded_directories(std_dirs: &StandardDirectories, args_excluded_dirs: &Option<Vec<String>>) -> Vec<String> {
    let mut excluded_dirs: Vec<String> = Vec::new();
    if let Some(dirs) = args_excluded_dirs {
        for dir in dirs {
            excluded_dirs.push(format!("{}/{}", std_dirs.home(), dir));
        }
    }
    for dir in DEFAULT_DIRECTORIES {
        excluded_dirs.push(format!("{}/{}", std_dirs.home(), dir));
    }
    for dir in std_dirs.all_subdirectories() {
        excluded_dirs.push(dir.to_string());
    }
    return excluded_dirs;
}

pub fn create_db_file(std_dirs: &StandardDirectories, user: &str) -> String {
    let db_path = get_db_path(&std_dirs);
    println!("Opening database at {db_path:?}");
    let already_existed = std::fs::exists(&db_path).expect("Failed to check if DB file exists");
    OpenOptions::new()
        .write(true)
        .create(true)
        .open(&db_path)
        .expect("Failed to open DB file");
    // Set file ownership back to user
    if !already_existed {
        let chown_output = Command::new("chown")
            .arg(format!("{}:{}", user, user))
            .arg(&db_path)
            .output();
        let success = chown_output.map(|out| out.status.success()).unwrap_or(false);
        if !success {
            eprintln!("Failed to set ownership of DB file at {db_path:?} to the user '{user}'");
        }
    }
    db_path
}
