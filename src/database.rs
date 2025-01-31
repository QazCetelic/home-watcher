use rusqlite::Connection;
use crate::ausearch_parse::Interaction;
use crate::time::{Date, DateTime, Time};

pub fn open_db(path: &str) -> Connection {
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

pub fn query_latest_time(conn: &Connection) -> Result<Option<DateTime>, String> {
    let mut stmt = conn.prepare("SELECT MAX(changed_at) as latest FROM files").map_err(|_| "Failed to prepare query".to_string())?;
    let mut rows = stmt.query([]).map_err(|_| "Failed to execute query")?;
    let row = rows.next().map_err(|_| "Failed to get row")?.unwrap();
    let datetime_int: Option<usize> = row.get(0).map_err(|_| "Failed to get column".to_string())?;
    let datetime = datetime_int.map(|int| DateTime::from_integer(int));
    Ok(datetime)
}