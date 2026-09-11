//! The storage half of the domain oracle (`domain-oracle/`), which recorded what the
//! C# pipeline did on each scenario. Not a port of a C# test: `DomainOracleTests`
//! produces these files, this reads them.
//!
//! - `expected/layout.json` names the store key and file of every layout id;
//! - `input.json`'s `store` and `expected/saved-store.json` hold the store documents
//!   before a load and after a save.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use lbm_store::json_format::{from_slice, to_string};
use lbm_store::layout_store_key::key_for;
use lbm_store::{GlobalOptionsDto, JsonLayoutStore, LayoutDto, ModelDto};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

fn scenarios() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(common::oracle_scenarios())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    assert!(dirs.len() >= 20, "oracle corpus not found: {dirs:?}");
    dirs
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&common::read_normalized(path))
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn store_keys_and_file_names_are_the_oracle_ones() {
    let config = Path::new("config");
    let store = JsonLayoutStore::new(config);

    for scenario in scenarios() {
        let layout = read_json(&scenario.join("expected/layout.json"));
        let id = layout["Id"].as_str().unwrap();

        assert_eq!(
            key_for(id),
            layout["StoreKey"].as_str().unwrap(),
            "{}",
            scenario.display()
        );

        let file = store.layout_path(id);
        let file = file.strip_prefix(config).unwrap();
        let expected: PathBuf = layout["StoreFile"].as_str().unwrap().split('/').collect();
        assert_eq!(file, expected, "{}", scenario.display());
    }
}

/// A store document of the oracle as its DTO, and what writing it gives back.
fn reserialize<T: Serialize + DeserializeOwned>(document: &Value) -> (T, Value) {
    let dto: T = from_slice(document.to_string().as_bytes()).unwrap();
    let written = to_string(&dto).unwrap();
    let reread: T = from_slice(written.as_bytes()).unwrap();
    assert_eq!(to_string(&reread).unwrap(), written);
    (dto, serde_json::from_str(&written).unwrap())
}

/// Re-serialize a store document keyed by its path in the store directory.
fn reserialize_file(name: &str, document: &Value) -> Value {
    match name {
        "options.json" => reserialize::<GlobalOptionsDto>(document).1,
        "models.json" => reserialize::<IndexMap<String, ModelDto>>(document).1,
        _ if name.starts_with("layouts/") => reserialize::<LayoutDto>(document).1,
        _ => panic!("unexpected store file {name}"),
    }
}

#[test]
fn store_documents_parse_and_the_saved_ones_come_back_unchanged() {
    let mut saved_documents = 0;
    for scenario in scenarios() {
        // Before the load: any shape a supported version wrote, the legacy ones
        // included, so only "it parses and its written form is stable" holds.
        let input = read_json(&scenario.join("input.json"));
        if let Some(store) = input.get("store").and_then(Value::as_object) {
            for (name, document) in store {
                reserialize_file(name, document);
            }
        }

        // After the save: what the C# writer produced. Writing what it parses to must
        // give the same JSON value back.
        let saved = read_json(&scenario.join("expected/saved-store.json"));
        for (name, document) in saved.as_object().unwrap() {
            assert_eq!(
                &reserialize_file(name, document),
                document,
                "{}: {name}",
                scenario.display()
            );
            saved_documents += 1;
        }
    }
    assert!(saved_documents >= 60, "{saved_documents}");
}
