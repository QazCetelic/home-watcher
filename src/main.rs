use crate::database::{open_db};
use crate::util::{get_default_db_path, get_excluded_directories, get_user, get_xdg_dirs};
use clap::{Parser, Subcommand};
use log::{debug, info};
use std::collections::HashSet;
use std::path::PathBuf;

mod file_audit;
mod audit_stream;
mod time;
mod util;
mod database;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Excluded directories (relative from user home)
    #[clap(short = 'd', long, value_delimiter = ' ', num_args = 1..)]
    pub excluded_dirs: Option<Vec<String>>, // relative to home dir

    /// Excluded executables
    #[clap(short = 'e', long, value_delimiter = ' ', num_args = 1..)]
    pub excluded_executables: Option<Vec<String>>,

    /// Directories to watch even though they are excluded by default (e.g. `.config`), relative to the user home
    #[clap(long, value_delimiter = ' ', num_args = 1..)]
    pub include_dirs: Option<Vec<String>>,

    /// Path for database file
    #[clap(short, long, default_value = get_default_db_path().into_os_string())]
    pub log_file: PathBuf,

    /// The user to watch the home directory of
    #[clap(short, long, default_value_t = get_user())]
    pub user: String,

    /// Path to auditctl binary
    #[clap(long, default_value = "/usr/sbin/auditctl", env = "HOME_WATCHER_AUDITCTL")]
    pub auditctl: String,

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
    env_logger::init();

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
    let xdg_dirs = get_xdg_dirs(&args.user);
    let conn = open_db(&xdg_dirs);
    let entries = database::query_all_entries(&conn);

    // Calculate column widths. Timestamps and count come first so the
    // variable-length paths stay at the end of the line.
    let mut max_added = "added_at".len();
    let mut max_changed = "changed_at".len();
    let mut max_count = "count".len();
    let mut max_file = "file".len();
    let mut max_exec = "executable".len();

    let formatted: Vec<(String, String, String, String, String)> = entries
        .iter()
        .map(|(file, executable, added_at, changed_at, count)| {
            let added_s = added_at.to_string();
            let changed_s = changed_at.to_string();
            let count_s = count.to_string();
            let file_s = file.clone();
            let exec_s = executable.clone();
            max_added = max_added.max(added_s.len());
            max_changed = max_changed.max(changed_s.len());
            max_count = max_count.max(count_s.len());
            max_file = max_file.max(file_s.len());
            max_exec = max_exec.max(exec_s.len());
            (added_s, changed_s, count_s, file_s, exec_s)
        })
        .collect();

    println!(
        "| {:<added_w$} | {:<changed_w$} | {:>count_w$} | {:<file_w$} | {:<exec_w$} |",
        "added_at",
        "changed_at",
        "count",
        "file",
        "executable",
        added_w = max_added,
        changed_w = max_changed,
        count_w = max_count,
        file_w = max_file,
        exec_w = max_exec,
    );
    for (added_at, changed_at, count, file, executable) in &formatted {
        println!(
            "| {:<added_w$} | {:<changed_w$} | {:>count_w$} | {:<file_w$} | {:<exec_w$} |",
            added_at,
            changed_at,
            count,
            file,
            executable,
            added_w = max_added,
            changed_w = max_changed,
            count_w = max_count,
            file_w = max_file,
            exec_w = max_exec,
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
    } else {
        excluded_executables_str = "".to_string();
    }
    info!("Excluding executables: {excluded_executables_str}");
    let xdg_dirs = get_xdg_dirs(&args.user);
    info!("Watching {}", xdg_dirs.home().display());
    let home = xdg_dirs.home().to_string_lossy().into_owned();
    let excluded_dirs = get_excluded_directories(
        &xdg_dirs,
        &args.excluded_dirs,
        &args.include_dirs,
    );
    let excluded_dirs_str = excluded_dirs.join(", ");
    info!("Excluding directories: {excluded_dirs_str}");
    let conn = open_db(&xdg_dirs);

    let auditd_rules = file_audit::generate_audit_rules(&home, &excluded_dirs);
    file_audit::ensure_audit_rules_exist(&auditd_rules, &args.auditctl)
        .expect("Failed to ensure audit rules are active");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        // Start the netlink audit listener (real-time, no polling) inside the runtime so that tokio::spawn() has an active context.
        let rx = audit_stream::spawn_audit_listener()
            .expect("Failed to create audit netlink listener (try running as root)");

        use tokio_stream::wrappers::UnboundedReceiverStream;
        use futures::StreamExt;

        // The kernel matches `-F dir=` against the whole syscall context and
        // then reports every path in it, so events can still carry paths from
        // outside the watched tree (e.g. /tmp, /proc). Enforce the boundary here.
        let home_prefix = format!("{}/", home.trim_end_matches('/'));

        let mut stream = UnboundedReceiverStream::new(rx);
        while let Some(interaction) = stream.next().await {
            if !interaction.file.starts_with(&home_prefix) {
                debug!("Ignoring path outside {}: {}", home, interaction.file);
                continue;
            }
            if excluded_dirs.iter().any(|dir| interaction.file.starts_with(dir)) {
                continue;
            }
            if excluded_executables.contains(&interaction.source) {
                continue;
            }
            database::add_entry(
                &conn,
                &interaction.file,
                &interaction.source,
                &interaction.date_time,
            );
        }
    });
}
