//! 전송 규약의 순수한 변환과 검증. 소켓, 파일, 시계, 런타임을 사용하지 않는다.

use crate::NetworkError;
use p2p_core::{MAX_DATAGRAM_BYTES,
               Message,
               MessageError};
use serde::{Deserialize,
            Serialize};
use sha2::{Digest,
           Sha256};

#[derive(Serialize, Deserialize)]
pub(super) struct Discovery {
    protocol: u8,
    pub(super) certificate: Vec<u8>,
    #[serde(flatten)]
    pub(super) message: Message,
}

impl Discovery {
    pub(super) fn decode(payload: &[u8]) -> Option<Self> {
        if payload.len() > MAX_DATAGRAM_BYTES {
            return None;
        }
        let discovery: Self = serde_json::from_slice(payload).ok()?;
        (discovery.protocol == 1 && discovery.certificate.len() <= 1024).then_some(discovery)
    }
}

pub(super) fn encode_discovery(message: &Message, certificate: &[u8], node_id: &str) -> Result<Vec<u8>, NetworkError> {
    if !matches!(message, Message::Hello { node_id: sender, .. } | Message::Goodbye { node_id: sender, .. } if sender == node_id) {
        return Err(NetworkError::Quic("invalid discovery message".into()));
    }
    let payload = serde_json::to_vec(&Discovery {
        protocol: 1,
        certificate: certificate.to_vec(),
        message: message.clone(),
    })
    .map_err(MessageError::from)?;
    if payload.len() > MAX_DATAGRAM_BYTES {
        return Err(MessageError::MessageTooLarge {
            size: payload.len(),
            max: MAX_DATAGRAM_BYTES,
        }
        .into());
    }
    Ok(payload)
}

pub(super) fn fingerprint(certificate: &[u8]) -> String { format!("{:x}", Sha256::digest(certificate)) }

pub(super) fn validate_frame_length(length: usize) -> Result<usize, NetworkError> {
    if length == 0 || length > MAX_DATAGRAM_BYTES {
        return Err(NetworkError::Quic("invalid chat frame length".into()));
    }
    Ok(length)
}

pub(super) fn decode_chat(payload: &[u8], node_id: &str) -> Result<Message, NetworkError> {
    let message = Message::decode(payload)?;
    if !matches!(&message, Message::Chat { node_id: sender, content, .. } if sender == node_id && !content.trim().is_empty()) {
        return Err(NetworkError::Quic("chat identity does not match certificate".into()));
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_roundtrip_preserves_wire_format_and_certificate() {
        for message in [Message::hello("peer", "한글", 9001), Message::goodbye("peer", "한글")] {
            let payload = encode_discovery(&message, &[1, 2, 3], "peer").unwrap();
            let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
            assert_eq!(json["protocol"], 1);
            assert!(json.get("type").is_some(), "the message must remain flattened");
            let discovery = Discovery::decode(&payload).unwrap();
            assert_eq!(discovery.message, message);
            assert_eq!(discovery.certificate, [1, 2, 3]);
        }
    }

    #[test]
    fn discovery_rejects_invalid_json_version_certificate_and_datagram_size() {
        assert!(Discovery::decode(b"invalid").is_none());
        assert!(Discovery::decode(&vec![b' '; MAX_DATAGRAM_BYTES + 1]).is_none());
        let payload = encode_discovery(&Message::hello("peer", "test", 9001), &[], "peer").unwrap();
        let mut json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        json["protocol"] = 2.into();
        assert!(Discovery::decode(&serde_json::to_vec(&json).unwrap()).is_none());
        json["protocol"] = 1.into();
        json["certificate"] = serde_json::to_value(vec![0; 1025]).unwrap();
        assert!(Discovery::decode(&serde_json::to_vec(&json).unwrap()).is_none());
    }

    #[test]
    fn outgoing_discovery_rejects_chat_wrong_identity_and_oversized_payload() {
        assert!(encode_discovery(&Message::chat("peer", "test", "chat"), &[], "peer").is_err());
        assert!(encode_discovery(&Message::goodbye("other", "test"), &[], "peer").is_err());
        assert!(matches!(
            encode_discovery(&Message::hello("peer", "가".repeat(MAX_DATAGRAM_BYTES), 9001), &[], "peer"),
            Err(NetworkError::Message(MessageError::MessageTooLarge { .. }))
        ));
    }

    #[test]
    fn frame_length_boundaries_are_checked_before_allocation() {
        assert!(validate_frame_length(0).is_err());
        assert_eq!(validate_frame_length(1).unwrap(), 1);
        assert_eq!(validate_frame_length(MAX_DATAGRAM_BYTES).unwrap(), MAX_DATAGRAM_BYTES);
        assert!(validate_frame_length(MAX_DATAGRAM_BYTES + 1).is_err());
    }

    #[test]
    fn chat_requires_authenticated_sender_and_nonblank_content() {
        let message = Message::chat("peer", "test", "한글 😀");
        assert_eq!(decode_chat(&message.encode().unwrap(), "peer").unwrap(), message);
        for message in [
            Message::chat("forged", "test", "text"),
            Message::chat("peer", "test", " \n"),
            Message::goodbye("peer", "test"),
        ] {
            assert!(decode_chat(&message.encode().unwrap(), "peer").is_err());
        }
        assert!(decode_chat(b"invalid", "peer").is_err());
    }
}
