/*
SELECT year, month, day, hour, minute, second FROM interactions
ORDER BY year DESC, month DESC, day DESC, hour DESC, minute DESC, second DESC
LIMIT 1
 */
use rusqlite::Connection;
use crate::ausearch_parse::Interaction;
use crate::time::{Date, DateTime, Time};

pub fn open_db(path: &str) -> Connection {
    let conn = Connection::open(path).expect("Failed to open database");
    conn.execute(
        "CREATE TABLE IF NOT EXISTS interactions (
            id          INTEGER PRIMARY KEY,
            year        INTEGER NOT NULL,
            month       INTEGER NOT NULL,
            day         INTEGER NOT NULL,
            hour        INTEGER NOT NULL,
            minute      INTEGER NOT NULL,
            second      INTEGER NOT NULL,
            file        TEXT NOT NULL,
            source      TEXT NOT NULL
        )",
        (),
    ).expect("Failed to create table");
    return conn;
}

pub fn insert_interaction(conn: &Connection, interaction: &Interaction) {
    conn.execute("
        INSERT INTO interactions (year, month, day, hour, minute, second, file, source)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    ", (&interaction.date().year(), &interaction.date().month(), &interaction.date().day(), &interaction.time().hour(), &interaction.time().minute(), &interaction.time().second(), &interaction.file(), &interaction.source()),
    ).expect("Failed to insert interaction into database");
}

pub fn query_latest_interaction_time(connection: &Connection) -> Result<Option<DateTime>, String> {
    let mut stmt = connection.prepare("
        SELECT year, month, day, hour, minute, second FROM interactions
        ORDER BY year DESC, month DESC, day DESC, hour DESC, minute DESC, second DESC
        LIMIT 1
    ").map_err(|_| "Failed to prepare query".to_string())?;
    let mut rows = stmt.query([]).map_err(|_| "Failed to execute query")?;
    let row = rows.next().map_err(|_| "Failed to get row")?.unwrap();
    let date = DateTime {
        date: Date::from_ymd(
            row.get(0).map_err(|_| "Failed to get year".to_string())?,
            row.get(1).map_err(|_| "Failed to get month".to_string())?,
            row.get(2).map_err(|_| "Failed to get day".to_string())?
        ),
        time: Time::from_hms(
            row.get(3).map_err(|_| "Failed to get hour".to_string())?,
            row.get(4).map_err(|_| "Failed to get minute".to_string())?,
            row.get(5).map_err(|_| "Failed to get second".to_string())?,
        )
    };
    return Ok(Some(date));
}