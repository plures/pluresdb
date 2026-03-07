use pluresdb_sync::GunMessage;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct FixtureExpect {
    #[serde(rename = "type")]
    message_type: Option<String>,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    name: Option<String>,
    raw: Option<Value>,
    expect: Option<FixtureExpect>,
}

fn fixture_dir() -> PathBuf {
    // crates/pluresdb-sync -> ../../tests/fixtures/gun-wire
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/gun-wire")
        .canonicalize()
        .expect("fixture dir should exist")
}

fn load_fixture_cases() -> anyhow::Result<Vec<FixtureCase>> {
    let dir = fixture_dir();
    let mut all_cases = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map(|ext| ext == "json")
                .unwrap_or(false)
        })
        .collect();
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let raw = fs::read_to_string(&path)?;
        let value: Value = serde_json::from_str(&raw)?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed")
            .to_string();

        match value {
            Value::Array(cases) => {
                for case in cases {
                    all_cases.push(serde_json::from_value(case)?);
                }
            }
            Value::Object(obj) => {
                // Support two fixture shapes:
                // 1) structured case: { name, raw, expect }
                // 2) raw wire message: {"#":..., "put"|"get"|"@":...}
                if obj.contains_key("raw") || obj.contains_key("name") || obj.contains_key("expect") {
                    all_cases.push(serde_json::from_value(Value::Object(obj))?);
                } else {
                    all_cases.push(FixtureCase {
                        name: Some(stem),
                        raw: Some(Value::Object(obj)),
                        expect: None,
                    });
                }
            }
            other => {
                anyhow::bail!(
                    "fixture file {} must contain object or array, got {}",
                    path.display(),
                    other
                );
            }
        }
    }

    if all_cases.is_empty() {
        anyhow::bail!("no gun wire fixture cases found in {}", dir.display());
    }

    Ok(all_cases)
}

#[test]
fn gun_wire_golden_decode_encode_invariants() {
    let cases = load_fixture_cases().expect("load fixtures");

    for (idx, case) in cases.into_iter().enumerate() {
        let case_name = case
            .name
            .clone()
            .unwrap_or_else(|| format!("fixture-case-{idx}"));
        let case_raw = case
            .raw
            .clone()
            .unwrap_or_else(|| panic!("case `{}` missing `raw` field", case_name));
        let raw_bytes = serde_json::to_vec(&case_raw).expect("serialize fixture raw payload");

        let decoded = GunMessage::decode(&raw_bytes)
            .unwrap_or_else(|err| panic!("case `{}` failed decode: {err:#}", case_name));

        if let Some(expect) = &case.expect {
            if let Some(ty) = &expect.message_type {
                assert_eq!(
                    decoded.message_type(),
                    ty,
                    "case `{}` message type mismatch",
                    case_name
                );
            }
            if let Some(id) = &expect.id {
                assert_eq!(decoded.id(), id, "case `{}` message id mismatch", case_name);
            }
        }

        let reencoded = decoded
            .encode()
            .unwrap_or_else(|err| panic!("case `{}` failed encode: {err:#}", case_name));

        let decoded_again = GunMessage::decode(&reencoded).unwrap_or_else(|err| {
            panic!(
                "case `{}` failed decode after re-encode: {err:#}",
                case_name
            )
        });

        assert_eq!(
            decoded, decoded_again,
            "case `{}` decode/encode/decode invariant failed",
            case_name
        );
    }
}
