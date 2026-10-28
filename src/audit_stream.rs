//! Receives audit events via netlink multicast (`AUDIT_NLGRP_READLOG`).
//!
//! Unlike `enable_events()`/`set_pid()` this does not take over as auditd's
//! receiver, so auditd keeps working. Joining the group requires `CAP_AUDIT_READ`.

use crate::time::{Date, DateTime, Time};
use linux_audit_parser::{MessageType, Number, Parser, Value};
use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use tokio::io::unix::AsyncFd;

const AUDIT_SYSCALL: u16 = 1300;
const AUDIT_PATH: u16 = 1302;

/// A file interaction assembled from a `SYSCALL` + `PATH` record pair.
#[derive(Debug, Clone)]
pub struct RawInteraction {
    pub date_time: DateTime,
    pub file: String,
    pub source: String,
}

fn value_to_string(value: &Value<'_>) -> Option<String> {
    match value {
        Value::Str(s, _) => Some(String::from_utf8_lossy(s).into_owned()),
        Value::Owned(v) => Some(String::from_utf8_lossy(v).into_owned()),
        Value::Segments(segs) => Some(segs.iter().map(|s| String::from_utf8_lossy(s)).collect()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn field_str<'a>(body: &linux_audit_parser::Body<'a>, key: &str) -> Option<String> {
    body.get(key).and_then(value_to_string)
}

fn field_u64(body: &linux_audit_parser::Body<'_>, key: &str) -> Option<u64> {
    match body.get(key)? {
        Value::Number(Number::Dec(v)) => u64::try_from(*v).ok(),
        Value::Number(Number::Hex(v)) | Value::Number(Number::Oct(v)) => Some(*v),
        other => value_to_string(other)?.parse().ok(),
    }
}

/// Convert a millisecond Unix timestamp to local time, like `ausearch` did.
fn datetime_from_millis(millis: u64) -> Option<DateTime> {
    let time_t = (millis / 1000) as libc::time_t;

    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::localtime_r(&time_t, &mut tm) };
    if result.is_null() {
        return None;
    }

    Some(DateTime {
        date: Date::from_ymd(
            (tm.tm_year + 1900) as usize,
            (tm.tm_mon + 1) as usize,
            tm.tm_mday as usize,
        ),
        time: Time::from_hms(
            tm.tm_hour as usize,
            tm.tm_min as usize,
            tm.tm_sec as usize,
        ),
    })
}

/// A `SYSCALL` record waiting for its `PATH` records.
#[derive(Debug, Clone)]
struct SyscallInfo {
    exe: String,
    timestamp_ms: u64,
}

/// Number of event sequences to keep `SYSCALL` records for.
///
/// A syscall can emit several `PATH` records (a rename has two). The `items=`
/// field is not used for this: it disagrees with the record count in several
/// cases and silently dropped interactions.
const CORRELATION_WINDOW: u32 = 4096;

/// Hard cap, in case sequences are not monotonic.
const MAX_PENDING: usize = 8192;

/// Correlates `AUDIT_SYSCALL` and `AUDIT_PATH` records by event id.
pub struct AuditEventParser {
    parser: Parser,
    pending: HashMap<u32, SyscallInfo>,
}

impl Default for AuditEventParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditEventParser {
    pub fn new() -> Self {
        Self {
            // Netlink delivers the raw (non-enriched) format, and we do not need `msg='…'` splitting for SYSCALL/PATH records.
            parser: Parser {
                enriched: false,
                split_msg: false,
            },
            pending: HashMap::new(),
        }
    }

    /// `msg_type` is the netlink header's `nlmsg_type`, `body` the raw payload,
    /// e.g. `audit(1734567890.123:456): arch=c000003e syscall=257 ...`.
    pub fn feed(&mut self, msg_type: u16, body: &str) -> Option<RawInteraction> {
        if msg_type != AUDIT_SYSCALL && msg_type != AUDIT_PATH {
            return None;
        }

        // Rebuild an auditd-style log line: "type=<NAME> msg=audit(…): …".
        let trimmed = body.trim_end_matches(['\0', '\n', '\r', ' ']);
        let line = format!(
            "type={} msg={}\n",
            MessageType(u32::from(msg_type)),
            trimmed
        );

        let message = match self.parser.parse(line.as_bytes()) {
            Ok(m) => m,
            Err(e) => {
                log::debug!("failed to parse audit record ({msg_type}): {e}: {trimmed}");
                return None;
            }
        };

        match msg_type {
            AUDIT_SYSCALL => self.on_syscall(&message),
            AUDIT_PATH => self.on_path(&message),
            _ => unreachable!(),
        }
    }

    fn on_syscall(&mut self, message: &linux_audit_parser::Message<'_>) -> Option<RawInteraction> {
        let key = field_str(&message.body, "key")?;
        if key != crate::file_audit::AUDITD_RULE_TAG {
            return None;
        }

        // Only successful operations are recorded.
        if field_str(&message.body, "success").as_deref() != Some("yes") {
            return None;
        }

        let exe = match field_str(&message.body, "exe") {
            Some(exe) => exe,
            None => {
                log::debug!("SYSCALL record without exe: {:?}", message.body);
                return None;
            }
        };

        let sequence = message.id.sequence;
        log::trace!("audit syscall {} by {exe}", field_u64(&message.body, "syscall").unwrap_or_default());

        if self.pending.len() >= MAX_PENDING {
            log::warn!("audit correlation cache full, dropping entries");
            self.pending.clear();
        } else {
            self.pending
                .retain(|seq, _| sequence.wrapping_sub(*seq) <= CORRELATION_WINDOW);
        }

        self.pending.insert(
            sequence,
            SyscallInfo {
                exe,
                timestamp_ms: message.id.timestamp,
            },
        );

        None
    }

    fn on_path(&mut self, message: &linux_audit_parser::Message<'_>) -> Option<RawInteraction> {
        // `PARENT` is a synthetic record for the containing directory.
        let nametype = field_str(&message.body, "nametype").unwrap_or_default();
        if nametype == "PARENT" || nametype == "UNKNOWN" {
            return None;
        }

        // Not removed: one syscall can produce several real `PATH` records.
        let info = self.pending.get(&message.id.sequence)?;

        let file = field_str(&message.body, "name")?;

        if file.starts_with("/proc/self/fd/") {
            log::warn!(
                "dropping unresolvable audited path {} (nametype={}, exe={})",
                file,
                nametype,
                info.exe
            );
            return None;
        }

        let interaction = RawInteraction {
            date_time: datetime_from_millis(info.timestamp_ms)?,
            file,
            source: info.exe.clone(),
        };

        log::debug!(
            "{}: {} -> {} (nametype={})",
            interaction.date_time.to_string(),
            interaction.source,
            interaction.file,
            nametype
        );

        Some(interaction)
    }
}

/// Multicast group that receives every event the kernel logs.
const AUDIT_NLGRP_READLOG: u32 = 1;

fn create_multicast_socket() -> io::Result<OwnedFd> {
    // SAFETY: the fd is wrapped in an `OwnedFd` immediately, so it is closed exactly once.
    let fd = unsafe {
        let fd = libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            libc::NETLINK_AUDIT,
        );
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        OwnedFd::from_raw_fd(fd)
    };

    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as u16;
    addr.nl_groups = AUDIT_NLGRP_READLOG;
    addr.nl_pid = 0;

    let ret = unsafe {
        libc::bind(
            fd.as_raw_fd(),
            &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(fd)
}

/// `sizeof(struct nlmsghdr)`
const NLMSG_HDRLEN: usize = 16;

/// Spawn a task reading audit events into the returned channel.
///
/// Must be called from within a tokio runtime.
pub fn spawn_audit_listener() -> io::Result<tokio::sync::mpsc::UnboundedReceiver<RawInteraction>> {
    let fd = create_multicast_socket()?;

    // SAFETY: the fd stays valid for as long as `async_fd`, which is moved into the task below.
    let async_fd = unsafe { AsyncFd::register(fd)? };

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut parser = AuditEventParser::new();
        let mut buf = vec![0u8; 64 * 1024];

        loop {
            let mut guard = match async_fd.readable().await {
                Ok(g) => g,
                Err(e) => {
                    log::error!("audit netlink socket closed: {e}");
                    break;
                }
            };

            let n = match guard.try_io(|inner| {
                // SAFETY: `buf` is a valid, exclusively borrowed buffer.
                let ret = unsafe {
                    libc::recv(
                        inner.get_ref().as_raw_fd(),
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                        0,
                    )
                };
                if ret < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(ret as usize)
                }
            }) {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    log::error!("audit netlink recv error: {e}");
                    break;
                }
                Err(_would_block) => continue,
            };

            if n < NLMSG_HDRLEN {
                continue;
            }

            let mut offset = 0usize;
            while offset + NLMSG_HDRLEN <= n {
                let nlmsg_len =
                    u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap()) as usize;
                let nlmsg_type =
                    u16::from_ne_bytes(buf[offset + 4..offset + 6].try_into().unwrap());

                if nlmsg_len < NLMSG_HDRLEN || offset + nlmsg_len > n {
                    break;
                }

                // 1..=4 are netlink control messages; anything above is an audit record type such as AUDIT_SYSCALL or AUDIT_PATH.
                if nlmsg_type > 4 {
                    let payload = &buf[offset + NLMSG_HDRLEN..offset + nlmsg_len];
                    let text = String::from_utf8_lossy(payload);
                    if let Some(interaction) = parser.feed(nlmsg_type, &text) {
                        if tx.send(interaction).is_err() {
                            // Receiver dropped — shut down.
                            return;
                        }
                    }
                }

                offset += (nlmsg_len + 3) & !3;
            }
        }
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSCALL_LINE: &str = concat!(
        "audit(1737809703.123:42): arch=c000003e syscall=257 success=yes exit=3 ",
        "items=1 ppid=1 pid=100 auid=1000 uid=1000 comm=\"cat\" ",
        "exe=\"/usr/bin/cat\" key=\"home_watcher_rule\""
    );

    const PATH_LINE: &str = concat!(
        "audit(1737809703.123:42): item=0 name=\"/home/user/test.txt\" ",
        "inode=123 dev=08:01 mode=0100644 ouid=1000 ogid=1000 nametype=CREATE"
    );

    #[test]
    fn correlates_syscall_and_path() {
        let mut p = AuditEventParser::new();

        assert!(p.feed(AUDIT_SYSCALL, SYSCALL_LINE).is_none());

        let interaction = p.feed(AUDIT_PATH, PATH_LINE).expect("interaction");
        assert_eq!(interaction.file, "/home/user/test.txt");
        assert_eq!(interaction.source, "/usr/bin/cat");

        // 1737809703 is 2025-01-25.
        assert_eq!(interaction.date_time.date, Date::from_ymd(2025, 1, 25));
    }

    #[test]
    fn emits_every_path_of_a_syscall() {
        let mut p = AuditEventParser::new();
        assert!(p.feed(AUDIT_SYSCALL, SYSCALL_LINE).is_none());

        let first = p.feed(AUDIT_PATH, PATH_LINE).expect("first path");
        assert_eq!(first.file, "/home/user/test.txt");

        let second_line = PATH_LINE.replace("/home/user/test.txt", "/home/user/other.txt");
        let second = p.feed(AUDIT_PATH, &second_line).expect("second path");
        assert_eq!(second.file, "/home/user/other.txt");
    }

    #[test]
    fn decodes_hex_encoded_names() {
        let mut p = AuditEventParser::new();
        assert!(p.feed(AUDIT_SYSCALL, SYSCALL_LINE).is_none());

        // "737020616365" == "sp ace".
        let hex_path = PATH_LINE.replace("name=\"/home/user/test.txt\"", "name=737020616365");
        let interaction = p.feed(AUDIT_PATH, &hex_path).expect("interaction");
        assert_eq!(interaction.file, "sp ace");
    }

    #[test]
    fn drops_unresolvable_proc_fd_paths() {
        let mut p = AuditEventParser::new();
        assert!(p.feed(AUDIT_SYSCALL, SYSCALL_LINE).is_none());

        let proc_path =
            PATH_LINE.replace("name=\"/home/user/test.txt\"", "name=\"/proc/self/fd/5\"");
        assert!(p.feed(AUDIT_PATH, &proc_path).is_none());
    }

    #[test]
    fn skips_parent_pseudo_entries() {
        let mut p = AuditEventParser::new();
        assert!(p.feed(AUDIT_SYSCALL, SYSCALL_LINE).is_none());

        let parent = PATH_LINE
            .replace("name=\"/home/user/test.txt\"", "name=\"/home/user\"")
            .replace("nametype=CREATE", "nametype=PARENT");
        assert!(p.feed(AUDIT_PATH, &parent).is_none());

        let interaction = p.feed(AUDIT_PATH, PATH_LINE).expect("interaction");
        assert_eq!(interaction.file, "/home/user/test.txt");
    }
}
