use crate::ausearch_parse::{parse_csv, Interaction};
use crate::time::DateTime;
use std::io::BufRead;
use std::process::{Command, Output, Stdio};

pub const AUDITD_RULE_TAG: &str = "home_watcher_rule";
const AUDITCTL_EXECUTABLE: &str = "/usr/sbin/auditctl";

pub fn generate_audit_rules(home_dir: &str, _excluded_dirs: &Vec<String>) -> Vec<String> {
    let mut rules = Vec::new();
    rules.push(format!("-a always,exit -F arch=b64 -F dir={home_dir}/.config -F perm=wa -k {AUDITD_RULE_TAG}"));
    // TODO add excludes for XDG and excluded dirs
    // rules.push(format!("-a never,exit -F path={xdg_directory}"))
    // rules.push(format!("-W never,exclude -F path={xdg_directory}"));
    return rules;
}

pub fn audit_rules_active(expected_rules: &Vec<String>) -> bool {
    let output: Output = Command::new(AUDITCTL_EXECUTABLE)
        .arg("-l")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .expect("Failed to execute auditctl");
    let mut active_rules = output.stdout.lines().filter_map(|r| r.ok());
    return expected_rules.iter().all(|er| active_rules.any(|ar| er == &ar ));
}

pub fn add_audit_rules(rules: &Vec<String>) -> Result<(), String> {
    for rule in rules {
        let mut process = Command::new(AUDITCTL_EXECUTABLE)
            .args(rule.split(" "))
            .spawn()
            .map_err(|e| e.to_string())?;
        process.wait().map_err(|e| e.to_string())?;
    }
    return Ok(());
}

pub fn read_logs(datetime: &Option<DateTime>) -> Vec<Interaction> {
    let mut command = Command::new("ausearch");
    command
        .arg("-k")
        .arg(AUDITD_RULE_TAG)
        .arg("--format")
        .arg("csv");
    if let Some(t) = datetime {
        // Only show entries since specified time
        command.arg("-ts").arg(t.date.to_ymd_string()).arg(t.time.to_hms_string());
    }
    let process = command
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to execute ausearch");
    let stdout = process.stdout.expect("Failed to get stdout");
    let reader = std::io::BufReader::new(stdout);
    let lines = reader.lines().map(|l| l.expect("Failed to read line"));
    let interactions = parse_csv(lines);
    return interactions;
}