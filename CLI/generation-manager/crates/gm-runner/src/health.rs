use crate::{error::{Error,
                    Result},
            process};
use gm_core::config::HealthCheck;
use std::{io::{Read,
               Write},
          net::{SocketAddr,
                TcpStream,
                ToSocketAddrs},
          os::unix::process::CommandExt,
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
        if Instant::now() >= deadline {
            return Err(Error::HealthTimeout(check.timeout_secs));
        }
        if probe_once(check, cwd, deadline) {
            return Ok(());
        }
        std::thread::sleep(interval.min(deadline.saturating_duration_since(Instant::now())));
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocalHealthVerifier;

impl gm_application::HealthVerifier for LocalHealthVerifier {
    fn verify(&self, check: &HealthCheck, cwd: &Path) -> gm_application::PortResult<()> { Ok(wait_until_healthy(check, cwd)?) }
}

fn probe_once(check: &HealthCheck, cwd: &Path, deadline: Instant) -> bool {
    if let Some(url) = &check.http
        && !http_ok(url, deadline)
    {
        return false;
    }
    if let Some(addr) = &check.tcp
        && !tcp_ok(addr, deadline)
    {
        return false;
    }
    if let Some(cmd) = &check.cmd
        && !command_ok(cmd, cwd, deadline)
    {
        return false;
    }
    true
}

fn resolve(addr: &str, deadline: Instant) -> Vec<SocketAddr> {
    if let Ok(addr) = addr.parse() {
        return vec![addr];
    }
    let addr = addr.to_string();
    let (sender, receiver) = std::sync::mpsc::channel();
    // System DNS lookup has no timeout; keep it outside the activation thread.
    std::thread::spawn(move || {
        let addresses = addr.to_socket_addrs().map(|addresses| addresses.collect()).unwrap_or_default();
        let _ = sender.send(addresses);
    });
    receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())).unwrap_or_default()
}

fn tcp_ok(addr: &str, deadline: Instant) -> bool {
    resolve(addr, deadline)
        .iter()
        .any(|addr| TcpStream::connect_timeout(addr, deadline.saturating_duration_since(Instant::now())).is_ok())
}

fn command_ok(cmd: &str, cwd: &Path, deadline: Instant) -> bool {
    let Ok(mut child) = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(cwd)
        .process_group(0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    loop {
        if Instant::now() >= deadline {
            let _ = process::terminate(&mut child);
            return false;
        }
        match child.try_wait() {
            | Ok(Some(status)) => return status.success(),
            | Ok(None) => std::thread::sleep(Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now()))),
            | Err(_) => {
                let _ = process::terminate(&mut child);
                return false;
            },
        }
    }
}

/// A deliberately tiny HTTP/1.0 GET: the probe targets a local process, so
/// pulling in a full client (and a TLS stack) would be all cost and no benefit.
fn http_ok(url: &str, deadline: Instant) -> bool {
    let Some((host_port, path)) = split_url(url) else {
        return false;
    };
    let Some(mut stream) = resolve(&host_port, deadline)
        .iter()
        .find_map(|addr| TcpStream::connect_timeout(addr, deadline.saturating_duration_since(Instant::now())).ok())
    else {
        return false;
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if stream.set_write_timeout(Some(remaining)).is_err() {
        return false;
    }

    let host = host_port.split(':').next().unwrap_or("localhost");
    let request = format!("GET {path} HTTP/1.0\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: gm\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut response = Vec::new();
    let mut buffer = [0u8; 512];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || stream.set_read_timeout(Some(remaining)).is_err() {
            return false;
        }
        let n = match stream.read(&mut buffer) {
            | Ok(0) | Err(_) => return false,
            | Ok(n) => n,
        };
        let newline = buffer[.. n].iter().position(|byte| *byte == b'\n');
        let line = &buffer[.. newline.map_or(n, |index| index + 1)];
        if response.len() + line.len() > 8192 {
            return false;
        }
        response.extend_from_slice(line);
        if newline.is_some() {
            break;
        }
    }

    let text = String::from_utf8_lossy(&response);
    let Some(status_line) = text.lines().next() else {
        return false;
    };
    let Some(code) = status_line.split_whitespace().nth(1) else {
        return false;
    };
    Instant::now() < deadline && matches!(code.parse::<u16>(), Ok(c) if (200..400).contains(&c))
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

    #[test]
    fn command_probe_cannot_hold_activation_past_its_deadline() {
        let start = std::time::Instant::now();
        assert!(!super::command_ok(
            "sleep 30 & wait",
            std::path::Path::new("/tmp"),
            start + std::time::Duration::from_millis(100)
        ));
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn a_silent_http_server_cannot_hold_activation_past_its_deadline() {
        use std::{net::TcpListener,
                  time::{Duration,
                         Instant}};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(300));
        });
        let start = Instant::now();
        assert!(!super::http_ok(&url, start + Duration::from_millis(100)));
        assert!(start.elapsed() < Duration::from_millis(250));
        server.join().unwrap();
    }

    #[test]
    fn http_status_line_can_arrive_in_multiple_reads() {
        use std::{io::{Read,
                       Write},
                  net::TcpListener,
                  time::{Duration,
                         Instant}};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/health", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).unwrap() > 0);
            stream.write_all(b"HTTP/1.0 ").unwrap();
            std::thread::sleep(Duration::from_millis(50));
            stream.write_all(b"200 OK\r\nContent-Length: 0\r\n\r\n").unwrap();
        });
        assert!(super::http_ok(&url, Instant::now() + Duration::from_secs(2)));
        server.join().unwrap();
    }

    #[test]
    fn http_status_line_has_a_size_limit() {
        use std::{io::{Read,
                       Write},
                  net::TcpListener,
                  time::{Duration,
                         Instant}};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).unwrap() > 0);
            let _ = stream.write_all(&vec![b'x'; 8193]);
        });
        assert!(!super::http_ok(&url, Instant::now() + Duration::from_secs(2)));
        server.join().unwrap();
    }
}
