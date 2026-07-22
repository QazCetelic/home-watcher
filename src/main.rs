use crate::database::open_db;
use crate::file_audit::read_logs;
use crate::standard_dirs::StandardDirectories;
use crate::time::DateTime;
use crate::user_env::UserEnvironment;
use crate::util::{create_db_file, get_default_db_path, get_excluded_directories, get_user};
use clap::{Parser, Subcommand};
use std::collections::HashSet;
use std::env::set_var;
use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

mod file_audit;
mod standard_dirs;
mod ausearch_parse;
mod time;
mod util;
mod database;
mod user_env;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Excluded directories (relative from user home)
    #[clap(short = 'd', long, value_delimiter = ' ', num_args = 1..)]
    pub excluded_dirs: Option<Vec<String>>, // relative to home dir

    /// Excluded executables
    #[clap(short = 'e', long, value_delimiter = ' ', num_args = 1..)]
    pub excluded_executables: Option<Vec<String>>,

    /// Path for database file
    #[clap(short, long, default_value = get_default_db_path().into_os_string())]
    pub log_file: PathBuf,

    /// Interval to check audit logs in milliseconds
    #[clap(short, long, default_value_t = 1500)]
    pub interval: u64,

    /// The user to watch the home directory of
    #[clap(short, long, default_value_t = get_user())]
    pub user: String,

    /// Path to auditctl binary
    #[clap(long, default_value = "/usr/sbin/auditctl", env = "HOME_WATCHER_AUDITCTL")]
    pub auditctl: String,

    /// Path to ausearch binary
    #[clap(long, default_value = "ausearch", env = "HOME_WATCHER_AUSEARCH")]
    pub ausearch: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show all entries
    Show,
    Monitor,
}

fn main() {
    // Use IS8601 / RFC3339 date (YYYY-MM-DD) to avoid issues with ausearch
    set_var("LC_TIME", "en_DK.UTF-8");

    let args = Args::parse();
    if let Some(command) = &args.command {
        match command {
            Commands::Show => show(args),
            Commands::Monitor => monitor(args),
        }
    }
    else {
        monitor(args);
    }
}

fn show(args: Args) {
    let user_env = UserEnvironment::from_user(&args.user).expect("Failed to get user env");
    let std_dirs = StandardDirectories::new(&args.user, &user_env);
    let db_file = create_db_file(&std_dirs);
    let conn = open_db(&db_file);
    let entries = database::query_all_entries(&conn);

    // Calculate column widths
    let mut max_file = "file".len();
    let mut max_exec = "executable".len();
    let mut max_added = "added_at".len();
    let mut max_changed = "changed_at".len();
    let mut max_count = "count".len();

    let formatted: Vec<(String, String, String, String, String)> = entries
        .iter()
        .map(|(file, executable, added_at, changed_at, count)| {
            let file_s = file.clone();
            let exec_s = executable.clone();
            let added_s = added_at.to_string();
            let changed_s = changed_at.to_string();
            let count_s = count.to_string();
            max_file = max_file.max(file_s.len());
            max_exec = max_exec.max(exec_s.len());
            max_added = max_added.max(added_s.len());
            max_changed = max_changed.max(changed_s.len());
            max_count = max_count.max(count_s.len());
            (file_s, exec_s, added_s, changed_s, count_s)
        })
        .collect();

    println!(
        "| {:<file_w$} | {:<exec_w$} | {:<added_w$} | {:<changed_w$} | {:>count_w$} |",
        "file",
        "executable",
        "added_at",
        "changed_at",
        "count",
        file_w = max_file,
        exec_w = max_exec,
        added_w = max_added,
        changed_w = max_changed,
        count_w = max_count,
    );
    for (file, executable, added_at, changed_at, count) in &formatted {
        println!(
            "| {:<file_w$} | {:<exec_w$} | {:<added_w$} | {:<changed_w$} | {:>count_w$} |",
            file,
            executable,
            added_at,
            changed_at,
            count,
            file_w = max_file,
            exec_w = max_exec,
            added_w = max_added,
            changed_w = max_changed,
            count_w = max_count,
        );
    }
}

fn monitor(args: Args) {
    let mut excluded_executables: HashSet<String> = Default::default();
    let excluded_executables_str: String;
    if let Some(excluded) = args.excluded_executables {
        excluded_executables_str = excluded.join(", ");
        for executable in excluded {
            excluded_executables.insert(executable);
        }
    }
    else {
        excluded_executables_str = "".to_string();
    }
    println!("Excluding directories: {excluded_executables_str}");
    let user_env = UserEnvironment::from_user(&args.user).expect("Failed to get user env");
    let std_dirs = StandardDirectories::new(&args.user, &user_env);
    println!("Watching {}", std_dirs.home());
    let excluded_dirs= get_excluded_directories(&std_dirs, &args.excluded_dirs);
    let excluded_dirs_str = excluded_dirs.join(", ");
    println!("Excluding directories: {excluded_dirs_str}");
    let db_file = create_db_file(&std_dirs);
    let conn = open_db(&db_file);

    let auditd_rules = file_audit::generate_audit_rules(std_dirs.home(), &excluded_dirs);
    let rule_active = file_audit::audit_rules_active(&auditd_rules, &args.auditctl);
    if !rule_active {
        file_audit::add_audit_rules(&auditd_rules, &args.auditctl).expect("Failed to add rule");
    }
    let mut datetime: Option<DateTime> = database::query_latest_time(&conn).unwrap_or(None);
    if let Some(dt) = &datetime {
        println!("Starting from {} {}", dt.date.to_ymd_string(), dt.time.to_hms_string());
    }
    loop {
        let interactions = read_logs(&datetime, &args.ausearch);
        'interactions: for interaction in &interactions {
            for excluded_dir in &excluded_dirs {
                if interaction.file().starts_with(excluded_dir) {
                    continue 'interactions;
                }
            }
            if excluded_executables.contains(interaction.source()) {
                continue 'interactions;
            }
            database::add_entry(&conn, interaction.file(), interaction.source(), interaction.datetime() );
        }
        datetime = interactions.last().map(|i| DateTime { date: i.date().clone(), time: i.time().clone() } );
        sleep(Duration::from_millis(args.interval));
    }
}