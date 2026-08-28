use crate::error::{Error,
                   Result};
use gm_core::config::HealthCheck;
use std::{io::{Read,
               Write},
          net::{TcpStream,
                ToSocketAddrs},
          path::Path,
          process::{Command,
                    Stdio},
          time::{Duration,
                 Instant}};

/// Poll the configured probe until it passes or the timeout expires.
///
/// This is the step that turns "switch and hope" into something you can trust:
/// a generation that never answers is rolled back automatically.
pub fn wait_until_healthy(check: &HealthCheck, cwd: &Path) -> Result<()> {
    if !check.is_configured() {
        return Ok(());
    }
    let timeout = Duration::from_secs(check.timeout_secs);
    let interval = Duration::from_secs(check.interval_secs.max(1));
    let deadline = Instant::now() + timeout;

    loop {
        if probe_once(check, cwd) {
            return Ok(());
        }
        if Instant::now() + interval > deadline {
            return Err(Error::HealthTimeout(check.timeout_secs));
        }
        std::thread::sleep(interval);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocalHealthVerifier;

impl gm_application::HealthVerifier for LocalHealthVerifier {
    fn verify(&self, check: &HealthCheck, cwd: &Path) -> gm_application::PortResult<()> { Ok(wait_until_healthy(check, cwd)?) }
}

fn probe_once(check: &HealthCheck, cwd: &Path) -> bool {
    if let Some(url) = &check.http
        && !http_ok(url)
    {
        return false;
    }
    if let Some(addr) = &check.tcp
        && !tcp_ok(addr)
    {
        return false;
    }
    if let Some(cmd) = &check.cmd
        && !command_ok(cmd, cwd)
    {
        return false;
    }
    true
}

fn tcp_ok(addr: &str) -> bool {
    let Ok(mut addrs) = addr.to_socket_addrs() else {
        return false;
    };
    addrs.any(|a| TcpStream::connect_timeout(&a, Duration::from_secs(2)).is_ok())
}

fn command_ok(cmd: &str, cwd: &Path) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// A deliberately tiny HTTP/1.0 GET: the probe targets a local process, so
/// pulling in a full client (and a TLS stack) would be all cost and no benefit.
fn http_ok(url: &str) -> bool {
    let Some((host_port, path)) = split_url(url) else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect(&host_port) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));

    let host = host_port.split(':').next().unwrap_or("localhost");
    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: gm\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = Vec::new();
    let mut buffer = [0u8; 512];
    // The status line is all we need; stop as soon as the first chunk lands.
    match stream.read(&mut buffer) {
        | Ok(0) | Err(_) => return false,
        | Ok(n) => response.extend_from_slice(&buffer[.. n]),
    }

    let text = String::from_utf8_lossy(&response);
    let Some(status_line) = text.lines().next() else {
        return false;
    };
    let Some(code) = status_line.split_whitespace().nth(1) else {
        return false;
    };
    matches!(code.parse::<u16>(), Ok(c) if (200..400).contains(&c))
}

/// `http://127.0.0.1:8080/healthz` -> (`127.0.0.1:8080`, `/healthz`)
fn split_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        | Some(i) => (&rest[.. i], &rest[i ..]),
        | None => (rest, "/"),
    };
    if authority.is_empty() {
        return None;
    }
    let host_port = if authority.contains(':') {
        authority.to_string()
    } else {
        format!("{authority}:80")
    };
    Some((host_port, path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::split_url;

    #[test]
    fn parses_url_with_port_and_path() {
        let (authority, path) = split_url("http://127.0.0.1:8080/healthz").unwrap();
        assert_eq!(authority, "127.0.0.1:8080");
        assert_eq!(path, "/healthz");
    }

    #[test]
    fn defaults_port_and_path() {
        let (authority, path) = split_url("http://localhost").unwrap();
        assert_eq!(authority, "localhost:80");
        assert_eq!(path, "/");
    }

    #[test]
    fn rejects_https() {
        assert!(split_url("https://example.com").is_none());
    }
}
