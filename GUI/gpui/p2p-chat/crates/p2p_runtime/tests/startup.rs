use p2p_app::{ConfigError,
              NodeConfig};
use p2p_runtime::{StartError,
                  start};
use std::{io::ErrorKind,
          net::UdpSocket};

#[test]
fn invalid_configuration_is_rejected_at_runtime_boundary() {
    assert!(matches!(
        start(NodeConfig {
            nickname: " ".into(),
            listen_port: 9001
        }),
        Err(StartError::Config(ConfigError::EmptyNickname))
    ));
    assert!(matches!(
        start(NodeConfig {
            nickname: "Alice".into(),
            listen_port: 0
        }),
        Err(StartError::Config(ConfigError::InvalidPort))
    ));
}

#[test]
fn occupied_port_preserves_io_error_for_gui() {
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = socket.local_addr().unwrap().port();
    assert!(matches!(start(NodeConfig::new("Alice", port).unwrap()), Err(StartError::Io(error)) if error.kind() == ErrorKind::AddrInUse));
}
