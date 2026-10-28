use std::process::{Command, Output};

pub const AUDITD_RULE_TAG: &str = "home_watcher_rule";
const AUDITD_EXCLUDE_TAG: &str = "home_watcher_exclude";

/// Rules watching the whole home tree except the excluded subtrees.
///
/// The `never` rules must come first: the kernel walks the exit filter list in
/// order and the first match wins. No `-F arch=` filter is used, which is also
/// how `auditctl -w` is implemented and keeps 32-bit callers covered.
pub fn generate_audit_rules(home_dir: &str, excluded_dirs: &[String]) -> Vec<String> {
    let mut rules: Vec<String> = excluded_dirs
        .iter()
        // Never suppress the root itself or anything outside it.
        .filter(|dir| *dir != home_dir && dir.starts_with(home_dir))
        .map(|dir| format!("-a never,exit -F dir={dir} -k {AUDITD_EXCLUDE_TAG}"))
        .collect();

    // `dir=` covers the whole subtree, `perm=wa` writes and attribute changes.
    rules.push(format!(
        "-a always,exit -F dir={home_dir} -F perm=wa -k {AUDITD_RULE_TAG}"
    ));

    rules
}

/// Rules live in kernel memory and survive process exit, so a restart would stack a new rule set on top of a stale one.
fn delete_our_rules(auditctl: &str) {
    for tag in [AUDITD_RULE_TAG, AUDITD_EXCLUDE_TAG] {
        match Command::new(auditctl).args(["-D", "-k", tag]).output() {
            Ok(output) if output.status.success() => {
                log::debug!("Deleted audit rules with key {tag}");
            }
            Ok(output) => log::debug!(
                "No audit rules deleted for key {tag}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
            Err(e) => log::warn!("Failed to run {auditctl} -D -k {tag}: {e}"),
        }
    }
}

/// Install the given rule set, replacing any previously installed one.
pub fn ensure_audit_rules_exist(rules: &[String], auditctl: &str) -> Result<(), String> {
    delete_our_rules(auditctl);

    for rule in rules {
        let output: Output = Command::new(auditctl)
            .args(rule.split_whitespace())
            .output()
            .map_err(|e| format!("Failed to execute auditctl: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // An already-present rule is not an error on older auditctl versions where `-D -k` is unsupported.
            if stderr.contains("Rule exists") || stderr.contains("already exists") {
                log::info!("Audit rule already present: {rule}");
                continue;
            }
            return Err(format!(
                "auditctl failed for rule '{rule}': {}",
                stderr.trim()
            ));
        }
        log::info!("Added audit rule: {rule}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excluded() -> Vec<String> {
        vec![
            "/home/user/.config".to_string(),
            "/home/user/.cache".to_string(),
            "/home/user/.local/share".to_string(),
            "/home/user/Downloads".to_string(),
        ]
    }

    #[test]
    fn generates_exclusions_then_the_watch_rule() {
        assert_eq!(
            generate_audit_rules("/home/user", &excluded()),
            vec![
                "-a never,exit -F dir=/home/user/.config -k home_watcher_exclude",
                "-a never,exit -F dir=/home/user/.cache -k home_watcher_exclude",
                "-a never,exit -F dir=/home/user/.local/share -k home_watcher_exclude",
                "-a never,exit -F dir=/home/user/Downloads -k home_watcher_exclude",
                "-a always,exit -F dir=/home/user -F perm=wa -k home_watcher_rule",
            ]
        );
    }

    #[test]
    fn skips_exclusions_outside_the_watched_root() {
        assert_eq!(
            generate_audit_rules(
                "/home/user",
                &["/etc".to_string(), "/home/user".to_string()],
            ),
            vec!["-a always,exit -F dir=/home/user -F perm=wa -k home_watcher_rule"]
        );
    }
}

