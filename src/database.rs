use log::{error, info};
use microxdg::Xdg;
use rusqlite::Connection;
use crate::{time::DateTime, util::get_db_path};
use std::{fs::OpenOptions, os::unix::fs::{MetadataExt, chown}, path::{PathBuf}};

pub fn open_db(xdg_dirs: &Xdg) -> Connection {
    let path = provide_db_file(xdg_dirs);
    let conn = Connection::open(path).expect("Failed to open database");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS files (
            file        TEXT NOT NULL,
            executable  TEXT NOT NULL,
            added_at    INTEGER NOT NULL,
            changed_at  INTEGER NOT NULL,
            count       INTEGER NOT NULL,
            PRIMARY KEY (file, executable)
        )",
        (),
    ).expect("Failed to create files table");
    return conn;
}

/// Returns a DB file path, creates file and sets permissions
fn provide_db_file(xdg_dirs: &Xdg) -> PathBuf {
    let db_path = get_db_path(xdg_dirs);

    OpenOptions::new()
        .write(true)
        .create(true)
        .open(&db_path)
        .expect("Failed to open DB file");
    
    let home_metadata = std::fs::metadata(xdg_dirs.home()).expect("Failed to stat home directory");
    let uid = home_metadata.uid();
    let gid = home_metadata.gid();
    if let Err(e) = chown(&db_path, Some(uid), Some(gid)) {
        error!("Failed to set ownership of DB file at {db_path:?} to UID {uid}:{gid}, error: {e:?}");
    }
    else {
        info!("Set ownership of DB file at {db_path:?} to UID {uid}:{gid}")
    }

    db_path
}

pub fn add_entry(conn: &Connection, file_path: &str, executable: &str, datetime: &DateTime) {
    conn.execute("
        INSERT OR REPLACE INTO files
        (file, executable, added_at, changed_at, count)
        VALUES (?1, ?2, ?3, ?3, 1)
        ON CONFLICT (file, executable) DO UPDATE SET
             changed_at = ?3,
             count = count + 1;
    ", (file_path, executable, datetime.as_integer()),
    ).expect("Failed to insert interaction into database");
}

pub fn query_all_entries(conn: &Connection) -> Vec<(String, String, DateTime, DateTime, usize)> {
    let mut stmt = conn.prepare("SELECT file, executable, added_at, changed_at, count FROM files").expect("Failed to prepare query");
    let rows = stmt.query_map([], |row| {
        let file: String = row.get(0)?;
        let executable: String = row.get(1)?;
        let added_at_int: usize = row.get(2)?;
        let changed_at_int: usize = row.get(3)?;
        let count: usize = row.get(4)?;
        Ok((file, executable, DateTime::from_integer(added_at_int), DateTime::from_integer(changed_at_int), count))
    }).expect("Failed to execute query");
    return rows.map(|r| r.expect("Failed to get row")).collect();
}
