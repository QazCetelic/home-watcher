use crate::file_audit::AUDITD_RULE_TAG;
use crate::time::{Date, Time};

const EXPECTED_HEADER: &str = "NODE,EVENT,DATE,TIME,SERIAL_NUM,EVENT_KIND,SESSION,SUBJ_PRIME,SUBJ_SEC,SUBJ_KIND,ACTION,RESULT,OBJ_PRIME,OBJ_SEC,OBJ_KIND,HOW";
const _COLUMN_NODE: usize = 0;
const _COLUMN_EVENT: usize = 1;
const COLUMN_DATE: usize = 2;
const COLUMN_TIME: usize = 3;
const _COLUMN_SERIAL: usize = 4;
const COLUMN_EVENT_KIND: usize = 5;
const _COLUMN_SESSION: usize = 6;
const _COLUMN_PRIME: usize = 7;
const _COLUMN_SEC: usize = 8;
const _COLUMN_SUBJ_KIND: usize = 9;
const COLUMN_ACTION: usize = 10;
const COLUMN_RESULT: usize = 11;
const COLUMN_OBJ_PRIME: usize = 12;
const _COLUMN_OBJ_SEC: usize = 13;
const _COLUMN_OBJ_KIND: usize = 14;
const COLUMN_HOW: usize = 15;

/// Parses ausearch with csv format option
pub fn parse_csv<I>(mut lines: I) -> Vec<Interaction>
where I: Iterator<Item = String>,
{
    let mut interactions: Vec<Interaction> = Vec::new();
    let header = lines.next().expect("Output is empty");
    if header != EXPECTED_HEADER {
        panic!("Unexpected header")
    }
    for row in lines {
        if let Some(interaction) = parse_row(row) {
            interactions.push(interaction);
        }
    }
    return interactions;
}

pub struct Interaction {
    date: Date,
    time: Time,
    /// File that was interacted with
    file: String,
    /// Executable that interacted with the file
    source: String,
}
impl Interaction {
    pub fn date(&self) -> &Date { &self.date }
    pub fn time(&self) -> &Time { &self.time }
    pub fn file(&self) -> &str { &self.file }
    pub fn source(&self) -> &str { &self.source }
}

fn parse_row(row: String) -> Option<Interaction> {
    // e.g.: ",SYSCALL,2025-01-25,12:55:03,235865,audit-rule,4,user,user,user-acct,opened-file,success,/home/user/.config/Nextcloud//logs/20250125_1255_nextcloud.log.2,51285974,file,/usr/bin/nextcloud"
    let columns = row.split(",").collect::<Vec<&str>>();

    let result = columns.get(COLUMN_RESULT)? == &"success";
    if !result {
        return None; // skip failures
    }
    let even_kind = columns.get(COLUMN_EVENT_KIND)?;
    if *even_kind != "audit-rule" {
        return None; // rule change or other non file related event
    }
    let action = columns.get(COLUMN_ACTION)?;
    let actions = vec!["opened-file", "deleted", "renamed", "changed-file-permissions-of", "created-directory", "add_rule"];
    if !actions.iter().any(|a| a == action) {
        return None; // rule change or other non file related event
    }
    let file = columns.get(COLUMN_OBJ_PRIME)?.to_string();
    let source = columns.get(COLUMN_HOW)?.to_string();
    if file == AUDITD_RULE_TAG {
        panic!("Parsing failed:\n{row}");
    }
    Some(Interaction {
        date: Date::from_str(columns.get(COLUMN_DATE)?)?,
        time: Time::from_str(columns.get(COLUMN_TIME)?)?,
        file: file,
        source: source,
    })
}

// test_parse_csv
#[test]
fn test_parse_csv() {
    let input = vec![
        "NODE,EVENT,DATE,TIME,SERIAL_NUM,EVENT_KIND,SESSION,SUBJ_PRIME,SUBJ_SEC,SUBJ_KIND,ACTION,RESULT,OBJ_PRIME,OBJ_SEC,OBJ_KIND,HOW".to_string(),
        ",SYSCALL,2025-01-25,12:55:03,235865,audit-rule,4,user,user,user-acct,opened-file,success,/home/user/.config/Nextcloud//logs/20250125_1255_nextcloud.log.2,51285974,file,/usr/bin/nextcloud".to_string(),
    ];
    let lines = input.into_iter();
    let interactions = parse_csv(lines);
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].date.year(), 2025);
    assert_eq!(interactions[0].date.month(), 1);
    assert_eq!(interactions[0].date.day(), 25);
    assert_eq!(interactions[0].time.hour(), 12);
    assert_eq!(interactions[0].time.minute(), 55);
    assert_eq!(interactions[0].time.second(), 3);
    assert_eq!(interactions[0].file, "/home/user/.config/Nextcloud//logs/20250125_1255_nextcloud.log.2");
    assert_eq!(interactions[0].source, "/usr/bin/nextcloud");
}
