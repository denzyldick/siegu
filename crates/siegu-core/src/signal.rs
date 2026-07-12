use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalMessage {
    #[serde(rename = "join")]
    Join { device_id: String },
    #[serde(rename = "joined")]
    Joined {
        device_id: String,
        room_id: String,
        peer_count: usize,
    },
    #[serde(rename = "offer")]
    Offer { payload: String, target: String },
    #[serde(rename = "answer")]
    Answer { payload: String, target: String },
    #[serde(rename = "ice_candidate")]
    IceCandidate { payload: String, target: String },
    #[serde(rename = "peer_disconnected")]
    PeerDisconnected { device_id: String },
    #[serde(rename = "peer_joined")]
    PeerJoined { device_id: String },
    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "create_room")]
    CreateRoom,
    #[serde(rename = "room_created")]
    RoomCreated { code: String },
    #[serde(rename = "join_room")]
    JoinRoom { code: String },
    #[serde(rename = "room_joined")]
    RoomJoined,
    #[serde(rename = "relay")]
    Relay {
        from: Option<String>,
        payload: serde_json::Value,
    },
    #[serde(rename = "room_closed")]
    RoomClosed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_join() {
        let msg = SignalMessage::Join {
            device_id: "abc".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"join""#));
        assert!(json.contains(r#""device_id":"abc""#));
    }

    #[test]
    fn test_deserialize_joined() {
        let json = r#"{"type":"joined","device_id":"d1","room_id":"r1","peer_count":2}"#;
        let msg: SignalMessage = serde_json::from_str(json).unwrap();
        match msg {
            SignalMessage::Joined {
                device_id,
                room_id,
                peer_count,
            } => {
                assert_eq!(device_id, "d1");
                assert_eq!(room_id, "r1");
                assert_eq!(peer_count, 2);
            }
            _ => panic!("Expected Joined"),
        }
    }

    #[test]
    fn test_roundtrip_all_variants() {
        let messages = vec![
            SignalMessage::Join {
                device_id: "d1".into(),
            },
            SignalMessage::Joined {
                device_id: "d1".into(),
                room_id: "r1".into(),
                peer_count: 2,
            },
            SignalMessage::Offer {
                payload: "sdp".into(),
                target: "peer".into(),
            },
            SignalMessage::Answer {
                payload: "sdp".into(),
                target: "peer".into(),
            },
            SignalMessage::IceCandidate {
                payload: "candidate".into(),
                target: "peer".into(),
            },
            SignalMessage::PeerDisconnected {
                device_id: "d1".into(),
            },
            SignalMessage::PeerJoined {
                device_id: "d1".into(),
            },
            SignalMessage::Error {
                message: "err".into(),
            },
            SignalMessage::CreateRoom,
            SignalMessage::RoomCreated { code: "abc".into() },
            SignalMessage::JoinRoom { code: "abc".into() },
            SignalMessage::RoomJoined,
            SignalMessage::Relay {
                from: Some("d1".into()),
                payload: serde_json::json!({"key": "val"}),
            },
            SignalMessage::RoomClosed,
        ];
        for msg in &messages {
            let json = serde_json::to_string(msg).unwrap();
            let decoded: SignalMessage = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&decoded).unwrap();
            assert_eq!(json, json2, "Roundtrip failed for {:?}", msg);
        }
    }

    #[test]
    fn test_reject_unknown_variant() {
        let json = r#"{"type":"unknown_type"}"#;
        assert!(serde_json::from_str::<SignalMessage>(json).is_err());
    }
}
