// Independent three-field serde model, NOT AI-Toolbox's consuming module.
use serde::Deserialize;
use std::{env, fs};

#[derive(Deserialize)]
struct Capsule {
    summary: String,
    model: String,
    created_at: i64,
}

#[derive(Deserialize)]
struct Fixture {
    name: String,
    plain_hex: String,
}

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0);
    (0..s.len()).step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("synthetic fixture hex"))
        .collect()
}

fn hex(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn main() {
    let path = env::args().nth(1).expect("synthetic corpus path");
    let fixtures: Vec<Fixture> = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    for fixture in fixtures {
        let parsed = serde_json::from_slice::<Capsule>(&unhex(&fixture.plain_hex));
        let result = match parsed {
            Ok(c) if !c.summary.trim().is_empty() => {
                let _ = (&c.model, c.created_at);
                serde_json::json!({"name": fixture.name, "accepted": true,
                    "summary_hex": hex(&c.summary), "trimmed_hex": hex(c.summary.trim())})
            }
            _ => serde_json::json!({"name": fixture.name, "accepted": false,
                "summary_hex": "", "trimmed_hex": ""}),
        };
        println!("{}", result);
    }
}
