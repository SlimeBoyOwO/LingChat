//! Content regression audit using the host's real condition/assignment semantics.
//! Simulates user choices and marker files; does not open windows or touch player data.
use anyhow::{Result, ensure};
use ling_chat_main::ai_service::game_system::script_engine::{
    events::evaluate_condition,
    utils::script_function::{apply_variable_action, parse_variable_action},
};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn enabled(event: &Value, vars: &Map<String, Value>) -> bool {
    event["condition"]
        .as_str()
        .is_none_or(|c| c.is_empty() || evaluate_condition(c, vars))
}
fn actions(option: &Value, vars: &mut Map<String, Value>) -> Result<()> {
    for action in option["actions"].as_array().into_iter().flatten() {
        if action["type"] == "set_var" {
            let (op, name, value) = parse_variable_action(action["content"].as_str().unwrap())?;
            let next = apply_variable_action(op, vars.get(&name), value);
            vars.insert(name, next);
        }
    }
    Ok(())
}
fn run(
    chapters: &BTreeMap<String, Value>,
    start: &str,
    vars: &mut Map<String, Value>,
    files: &mut BTreeSet<String>,
    ending: &str,
    tone: &str,
    rare: bool,
    visited: &mut BTreeSet<String>,
) -> Result<Vec<String>> {
    let mut chapter = start.to_owned();
    let mut route = Vec::new();
    for _ in 0..120 {
        if chapter == "end" {
            return Ok(route);
        }
        visited.insert(chapter.clone());
        route.push(chapter.clone());
        let content = chapters
            .get(&chapter)
            .ok_or_else(|| anyhow::anyhow!("Missing {chapter}"))?;
        let mut next = "end".to_string();
        for event in content["events"].as_array().unwrap() {
            if !enabled(event, vars) {
                continue;
            }
            match event["type"].as_str().unwrap() {
                "set_variable" => {
                    for option in event["options"].as_array().unwrap() {
                        if enabled(option, vars) {
                            actions(option, vars)?;
                        }
                    }
                },
                "random_var" => {
                    vars.insert(event["variable"].as_str().unwrap().into(), json!(rare));
                },
                "character_file" => {
                    let file = event["file"].as_str().unwrap_or("");
                    match event["action"].as_str().unwrap() {
                        "ensure" => {
                            files.insert(file.into());
                        },
                        "delete" => {
                            files.remove(file);
                        },
                        "exists" => {
                            vars.insert(
                                event["resultVar"]
                                    .as_str()
                                    .unwrap_or("character_file_exists")
                                    .into(),
                                json!(files.contains(file)),
                            );
                        },
                        "open_folder" if ending == "release" => {
                            files.remove("MAIN.chr");
                        },
                        _ => {},
                    }
                },
                "poem_game" => {
                    vars.insert(
                        event["resultVar"].as_str().unwrap_or("poem_tone").into(),
                        json!(tone),
                    );
                    vars.insert(
                        "poem_glitch".into(),
                        json!(rare && event["mode"] != "normal"),
                    );
                },
                "choices" | "force_choice" => {
                    let options: Vec<_> = event["options"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|o| enabled(o, vars))
                        .collect();
                    let selected = if let Some(forced) = event["forced"].as_str() {
                        options.iter().find(|o| o["text"] == forced).copied()
                    } else if ending == "loop"
                        && matches!(chapter.as_str(), "a3_delete_prompt" | "a4_legacy_recovery")
                    {
                        options.get(1).copied()
                    } else {
                        options.first().copied()
                    };
                    actions(
                        selected.ok_or_else(|| anyhow::anyhow!("No choice in {chapter}"))?,
                        vars,
                    )?;
                },
                "chapter_end" => {
                    if event["end_type"] == "branching" {
                        next = event["options"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|o| enabled(o, vars))
                            .and_then(|o| o["next"].as_str())
                            .unwrap_or("end")
                            .into();
                    } else {
                        next = event["next"]
                            .as_str()
                            .or(event["next_chapter"].as_str())
                            .unwrap_or("end")
                            .into();
                    }
                    break;
                },
                _ => {}, // Presentation is covered by schema and browser/native audits.
            }
        }
        chapter = next;
    }
    anyhow::bail!("Unbounded route: {route:?}; vars={vars:?}")
}
fn main() -> Result<()> {
    let directory = std::env::args()
        .nth(1)
        .expect("usage: story_routes SCRIPT_DIR");
    let mut chapters = BTreeMap::new();
    for entry in std::fs::read_dir(Path::new(&directory).join("Chapters"))? {
        let path = entry?.path();
        if path.extension().and_then(|x| x.to_str()) == Some("yaml") {
            chapters.insert(
                path.file_stem().unwrap().to_string_lossy().into_owned(),
                serde_yaml::from_str::<Value>(&std::fs::read_to_string(path)?)?,
            );
        }
    }
    let mut visited = BTreeSet::new();
    let mut scenarios = Vec::new();
    for ending in ["release", "loop"] {
        for tone in ["warm", "script", "void"] {
            for rare in [false, true] {
                let mut vars = Map::new();
                let mut files = BTreeSet::new();
                for act in 1..=4 {
                    vars.insert("playthrough".into(), json!(act));
                    let route = run(
                        &chapters,
                        "a1_boot",
                        &mut vars,
                        &mut files,
                        ending,
                        tone,
                        rare,
                        &mut visited,
                    )?;
                    ensure!(
                        vars.get("current_act") == Some(&json!(act + 1)),
                        "Act {act} failed: {route:?}, {vars:?}"
                    );
                }
                ensure!(
                    vars["last_ending"] == ending && vars["act4_done"] == true,
                    "Ending mismatch"
                );
                scenarios.push(format!("{ending}/{tone}/rare={rare}"));
            }
        }
    }
    // Replay interrupted marker transactions; an opening must not loop or skip an act.
    for (checkpoint, act) in [
        ("act1_to_act2", 2),
        ("act2_to_act3", 3),
        ("legacy_release", 4),
        ("legacy_loop", 4),
    ] {
        let mut vars = json!({"playthrough":7,"current_act":1,"marker_checkpoint":checkpoint})
            .as_object()
            .unwrap()
            .clone();
        let route = run(
            &chapters,
            "a1_boot",
            &mut vars,
            &mut BTreeSet::new(),
            "release",
            "script",
            false,
            &mut visited,
        )?;
        ensure!(
            vars["current_act"] == act + 1,
            "Checkpoint {checkpoint}: {route:?}"
        );
        scenarios.push(checkpoint.into());
    }
    for missing in ["MAIN.chr", "ql.chr"] {
        let mut vars = json!({"playthrough":3,"current_act":1,"marker_schema_version":1})
            .as_object()
            .unwrap()
            .clone();
        let mut files = BTreeSet::from(["MAIN.chr".into(), "ql.chr".into()]);
        files.remove(missing);
        let route = run(
            &chapters,
            "a1_boot",
            &mut vars,
            &mut files,
            "release",
            "warm",
            false,
            &mut visited,
        )?;
        ensure!(
            route.iter().any(|c| c.contains("early_delete")),
            "Missing marker was silently recreated"
        );
        scenarios.push(format!("early-delete/{missing}"));
    }
    // Explicit old-save entries, watcher jumps, and missing-marker recovery branches.
    // These are intentionally not all reachable during an ordinary four-act playthrough.
    for chapter in chapters
        .keys()
        .filter(|k| !visited.contains(*k))
        .cloned()
        .collect::<Vec<_>>()
    {
        let act = chapter[1..2].parse::<i64>().unwrap_or(2);
        let mut vars = json!({"playthrough":7,"current_act":act,"marker_schema_version":1,"last_ending":"release"}).as_object().unwrap().clone();
        run(
            &chapters,
            &chapter,
            &mut vars,
            &mut BTreeSet::from(["MAIN.chr".into(), "ql.chr".into()]),
            "release",
            "script",
            false,
            &mut visited,
        )?;
        scenarios.push(format!("recovery/{chapter}"));
    }
    ensure!(
        visited.len() == chapters.len(),
        "Not every chapter was exercised"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"scenarios":scenarios,"visited":visited}))?
    );
    Ok(())
}
