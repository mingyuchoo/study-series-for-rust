use thiserror::Error;

pub const MAX_NICKNAME_CHARS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    pub nickname: String,
    pub listen_port: u16,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("닉네임을 입력하세요.")]
    EmptyNickname,
    #[error("닉네임은 {MAX_NICKNAME_CHARS}자 이하로 입력하세요.")]
    NicknameTooLong,
    #[error("포트는 1~65535 사이의 숫자여야 합니다.")]
    InvalidPort,
}

impl NodeConfig {
    pub fn new(nickname: impl Into<String>, listen_port: u16) -> Result<Self, ConfigError> {
        let nickname = nickname.into().trim().to_string();
        if nickname.is_empty() {
            return Err(ConfigError::EmptyNickname);
        }
        if nickname.chars().count() > MAX_NICKNAME_CHARS {
            return Err(ConfigError::NicknameTooLong);
        }
        if listen_port == 0 {
            return Err(ConfigError::InvalidPort);
        }
        Ok(Self {
            nickname,
            listen_port,
        })
    }
}
