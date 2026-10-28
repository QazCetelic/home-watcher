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
    let conn = open_db(&get_xdg_dirs(&args.user));
    let mut rows = vec![[
        "added_at".to_string(),
        "changed_at".to_string(),
        "count".to_string(),
        "file".to_string(),
        "executable".to_string(),
    ]];
    rows.extend(database::query_all_entries(&conn).iter().map(|(file, exec, added, changed, count)| {
        [added.to_string(), changed.to_string(), count.to_string(), file.clone(), exec.clone()]
    }));

    let widths: [usize; 5] = std::array::from_fn(|i| rows.iter().map(|row| row[i].len()).max().unwrap_or(0));
    for row in &rows {
        println!(
            "| {:<w0$} | {:<w1$} | {:>w2$} | {:<w3$} | {:<w4$} |",
            row[0], row[1], row[2], row[3], row[4],
            w0 = widths[0], w1 = widths[1], w2 = widths[2], w3 = widths[3], w4 = widths[4],
        );
    }
}

fn monitor(args: Args) {
    let excluded = args.excluded_executables.unwrap_or_default();
    info!("Excluding executables: {}", excluded.join(", "));
    let excluded_executables: HashSet<String> = excluded.into_iter().collect();
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
