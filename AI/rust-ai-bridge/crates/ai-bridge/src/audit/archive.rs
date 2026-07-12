//! 감사 기록 아카이브 (삭제 전 내보내기).
//!
//! **아카이브는 선택이 아닙니다.** `Purge` 는 [`Exporter`] 를 반드시 받습니다.
//! 아카이브 없이 버리기로 **결정했다면** [`Discard`] 를 명시해야 합니다 —
//! "깜빡한 것"과 "그러기로 한 것"은 코드에서 구분되어야 합니다.
//!
//! **내보내기가 성공한 기록만 지워집니다.** 반대 순서(삭제 후 내보내기)를
//! 택하면 크래시 한 번에 기록이 영구히 사라집니다.

use super::Entry;
use anyhow::{Result,
             anyhow,
             bail};
use chrono::Utc;
use serde_json::{Map,
                 Value,
                 json};
use std::sync::Mutex;

/// 감사 기록을 외부로 내보냅니다.
///
/// **성공을 보고했으면 정말로 내구성 있게 저장되어야 합니다.** `Purge` 는
/// `export` 가 `Ok` 를 돌려준 기록만 삭제하므로, 조용히 유실되는 구현(UDP
/// syslog 등)은 곧 감사 기록 유실입니다.
#[async_trait::async_trait]
pub trait Exporter: Send + Sync {
    async fn export(&self, entries: &[Entry]) -> Result<()>;
}

/// 아카이브하지 않고 버립니다. **개발·테스트 전용.**
///
/// 반드시 명시적으로 선택해야 합니다(`-discard`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Discard;

#[async_trait::async_trait]
impl Exporter for Discard {
    async fn export(&self, _entries: &[Entry]) -> Result<()> { Ok(()) }
}

/// 외부로 나가는 안정적인 JSON 형태.
///
/// Go 구조체 필드명이 아니라 **손으로 적은 snake_case 키**입니다 — 내부 필드
/// 이름을 바꿔도 아카이브 포맷이 따라 바뀌지 않게 하기 위함입니다.
fn archive_record(e: &Entry) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("id".into(), json!(e.id));
    m.insert("timestamp".into(), json!(crate::clock::to_rfc3339(e.timestamp)));
    m.insert("actor".into(), json!(e.actor));
    m.insert("tool".into(), json!(e.tool));
    m.insert("system".into(), json!(e.system));
    m.insert("access".into(), json!(e.access));
    m.insert("decision".into(), json!(e.decision));
    m.insert("reason".into(), json!(e.reason));
    m.insert("approval_status".into(), json!(e.approval_status));
    m.insert("approval_id".into(), json!(e.approval_id));
    m.insert("request_id".into(), json!(e.request_id));
    m.insert("session_id".into(), json!(e.session_id));
    m.insert("masked".into(), json!(e.masked));
    m.insert("input".into(), json!(e.input));
    m.insert("output".into(), json!(e.output));
    m.insert("latency_ms".into(), json!(e.latency_ms));
    m.insert("input_tokens".into(), json!(e.input_tokens));
    m.insert("output_tokens".into(), json!(e.output_tokens));
    m.insert("cost_micros".into(), json!(e.cost_micros));
    m.insert("error".into(), json!(e.error));
    m.insert("prompt".into(), json!(e.prompt));
    m.insert("injection".into(), json!(e.injection));
    m
}

/// 날짜별 JSON Lines 파일 (권한 600).
///
/// object storage(S3/GCS/Blob)에 그대로 업로드할 수 있습니다.
///
/// **아카이브 파일에는 프롬프트 원문이 들어 있습니다**(출력은 마스킹된 값).
/// 접근 통제가 없는 곳에 두면 안 됩니다.
#[derive(Debug)]
pub struct FileExporter {
    dir: std::path::PathBuf,
    open: Mutex<Option<(String, std::fs::File)>>,
}

impl FileExporter {
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Result<Self> {
        let dir = dir.into();
        if dir.as_os_str().is_empty() {
            bail!("audit: archive dir is required");
        }
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            // 소유자만 — 프롬프트 원문이 들어 있습니다.
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            dir,
            open: Mutex::new(None),
        })
    }
}

#[async_trait::async_trait]
impl Exporter for FileExporter {
    async fn export(&self, entries: &[Entry]) -> Result<()> {
        use std::io::Write as _;

        if entries.is_empty() {
            return Ok(());
        }
        let day = Utc::now().format("%Y-%m-%d").to_string();
        let mut guard = self.open.lock().unwrap();

        // 날짜가 바뀌면 새 파일로 넘어갑니다.
        if guard.as_ref().map(|(d, _)| d != &day).unwrap_or(true) {
            let path = self.dir.join(format!("audit-{day}.jsonl"));
            let mut opts = std::fs::OpenOptions::new();
            opts.create(true).write(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                opts.mode(0o600);
            }
            let f = opts.open(&path)?;
            *guard = Some((day, f));
        }

        let (_, file) = guard.as_mut().unwrap();
        for e in entries {
            let line = serde_json::to_string(&Value::Object(archive_record(e)))?;
            writeln!(file, "{line}")?;
        }
        // 삭제는 export 가 성공한 뒤에 일어납니다. fsync 하지 않으면 "내보냈다"고
        // 보고한 기록이 크래시로 사라진 채 원본만 지워질 수 있습니다.
        file.sync_all()?;
        Ok(())
    }
}

/// RFC 5424 syslog → SIEM (Splunk · QRadar · Sentinel).
///
/// **UDP 를 쓰지 마십시오** — 조용히 유실되는데, 유실을 알 수 없으면 `export`
/// 가 성공했다고 보고하고 원본 기록이 지워집니다.
#[derive(Debug)]
pub struct SyslogExporter {
    addr: String,
    tag: String,
}

impl SyslogExporter {
    pub fn new(addr: impl Into<String>, tag: impl Into<String>) -> Result<Self> {
        let tag = tag.into();
        Ok(Self {
            addr: addr.into(),
            tag: if tag.is_empty() { "ai-bridge".to_string() } else { tag },
        })
    }
}

#[async_trait::async_trait]
impl Exporter for SyslogExporter {
    async fn export(&self, entries: &[Entry]) -> Result<()> {
        use tokio::io::AsyncWriteExt as _;

        if entries.is_empty() {
            return Ok(());
        }
        let mut out = String::new();
        for e in entries {
            let payload = serde_json::to_string(&Value::Object(archive_record(e)))?;
            // PRI 134 = facility local0(16)*8 + severity info(6).
            // 형식: <PRI>VERSION TIMESTAMP HOSTNAME APP-NAME PROCID MSGID SD MSG
            out.push_str(&format!("<134>1 {} - {} - - - {}\n", crate::clock::to_rfc3339(e.timestamp), self.tag, payload));
        }
        let mut conn = tokio::net::TcpStream::connect(&self.addr)
            .await
            .map_err(|e| anyhow!("audit: syslog connect {}: {e}", self.addr))?;
        conn.write_all(out.as_bytes()).await?;
        conn.flush().await?;
        Ok(())
    }
}

/// 테스트용: 무엇을 내보냈는지 기록합니다.
#[derive(Debug, Default)]
pub struct RecordingExporter {
    pub exported: Mutex<Vec<Entry>>,
}

#[async_trait::async_trait]
impl Exporter for RecordingExporter {
    async fn export(&self, entries: &[Entry]) -> Result<()> {
        self.exported.lock().unwrap().extend_from_slice(entries);
        Ok(())
    }
}

/// 테스트용: 항상 실패합니다. **아무것도 지워지면 안 됩니다.**
#[derive(Debug, Default)]
pub struct FailingExporter;

#[async_trait::async_trait]
impl Exporter for FailingExporter {
    async fn export(&self, _entries: &[Entry]) -> Result<()> { bail!("archive backend is down") }
}

/// 아카이브 대상 하나를 고릅니다. **정확히 하나여야 합니다.**
pub fn build_exporter(archive_dir: &str, syslog_addr: &str, discard: bool) -> Result<Box<dyn Exporter>> {
    let chosen = [!archive_dir.is_empty(), !syslog_addr.is_empty(), discard].iter().filter(|b| **b).count();
    match chosen {
        | 0 => bail!("아카이브 대상을 지정해야 합니다: -archive-dir, -syslog, 또는 -discard"),
        | 1 => {},
        | _ => bail!("아카이브 대상은 하나만 지정하세요"),
    }
    if !archive_dir.is_empty() {
        return Ok(Box::new(FileExporter::new(archive_dir)?));
    }
    if !syslog_addr.is_empty() {
        return Ok(Box::new(SyslogExporter::new(syslog_addr, "ai-bridge")?));
    }
    eprintln!("경고: 아카이브 없이 삭제합니다. 기록을 복구할 수 없습니다.");
    Ok(Box::new(Discard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn entry(id: i64) -> Entry {
        Entry {
            id,
            timestamp: Utc.with_ymd_and_hms(2026, 7, 10, 9, 0, 0).unwrap(),
            actor: "emp-sales-01".into(),
            tool: "get_invoice_status".into(),
            prompt: "INV-1 결제됐어?".into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn file_exporter_writes_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let exp = FileExporter::new(dir.path()).unwrap();
        exp.export(&[entry(1), entry(2)]).await.unwrap();

        let day = Utc::now().format("%Y-%m-%d").to_string();
        let body = std::fs::read_to_string(dir.path().join(format!("audit-{day}.jsonl"))).unwrap();
        let lines: Vec<&str> = body.trim().lines().collect();
        assert_eq!(lines.len(), 2);

        let v: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["id"], json!(1));
        assert_eq!(v["timestamp"], json!("2026-07-10T09:00:00Z"));
        // 아카이브 포맷은 snake_case 이며 프롬프트 원문을 담습니다.
        assert_eq!(v["prompt"], json!("INV-1 결제됐어?"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn archive_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let exp = FileExporter::new(dir.path()).unwrap();
        exp.export(&[entry(1)]).await.unwrap();

        let day = Utc::now().format("%Y-%m-%d").to_string();
        let md = std::fs::metadata(dir.path().join(format!("audit-{day}.jsonl"))).unwrap();
        assert_eq!(md.permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn build_exporter_requires_exactly_one_target() {
        // 깜빡한 것 — 거부합니다.
        assert!(build_exporter("", "", false).is_err());
        // 둘을 준 것 — 거부합니다.
        assert!(build_exporter("/tmp/a", "siem:514", false).is_err());
        // 그러기로 한 것 — 허용합니다.
        assert!(build_exporter("", "", true).is_ok());
    }

    #[tokio::test]
    async fn discard_succeeds_silently() {
        assert!(Discard.export(&[entry(1)]).await.is_ok());
    }

    #[tokio::test]
    async fn failing_exporter_reports_failure() {
        assert!(FailingExporter.export(&[entry(1)]).await.is_err());
    }
}
