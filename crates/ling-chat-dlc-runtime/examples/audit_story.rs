//! Validate every chapter against the host schema and exact installed media roots.
use ling_chat_main::ai_service::game_system::script_engine::{self, events, validate};
use std::{collections::HashMap, path::PathBuf};

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        args.len() == 3,
        "usage: audit_story HOST_DATA_DIR SCRIPT_DIR"
    );
    let data = PathBuf::from(&args[1]);
    let script = PathBuf::from(&args[2]);
    ling_chat_dlc_runtime::install();
    script_engine::init_event_registry();
    let report = validate::validate(&data, &script, "audit", &HashMap::new());
    let mut count = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(script.join("Chapters"))? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }
        let value: serde_json::Value = serde_yaml::from_str(&std::fs::read_to_string(&path)?)?;
        for event in value["events"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid chapter {path:?}"))?
        {
            let kind = event["type"].as_str().unwrap_or_default();
            anyhow::ensure!(
                events::create_event(kind, event.clone()).is_some(),
                "Unregistered {kind} in {path:?}"
            );
            kinds.insert(kind.to_owned());
        }
        count += 1;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"chapters":count,"event_types":kinds,"validation":report})
        )?
    );
    anyhow::ensure!(
        report.error_count == 0,
        "Story contains {} validation errors",
        report.error_count
    );
    Ok(())
}
