use pluresdb_wire_gun::GunMessage;
use std::fs;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../tests/fixtures/gun-wire");
    path.push(name);
    fs::read_to_string(path).expect("fixture should be readable")
}

#[test]
fn canonical_fixtures_round_trip() {
    for name in ["put.json", "get.json", "ack.json"] {
        let raw = fixture(name);
        let message: GunMessage = serde_json::from_str(&raw).expect("fixture should deserialize");
        let encoded = message.encode().expect("encode should succeed");
        let decoded = GunMessage::decode(&encoded).expect("decode should succeed");
        assert_eq!(message, decoded, "fixture {name} should round-trip");
    }
}
