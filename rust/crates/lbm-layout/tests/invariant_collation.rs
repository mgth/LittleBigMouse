//! `invariant_compare` against what .NET 10's `StringComparer.InvariantCulture`
//! answered on the same strings (tests/data/invariant-collation.json, produced
//! by tests/data/invariant-collation-generator).

use std::cmp::Ordering;

use lbm_layout::collation::invariant_compare;

fn vectors() -> serde_json::Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/invariant-collation.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("vectors")).expect("json")
}

#[test]
fn every_recorded_comparison_matches_dotnet() {
    let v = vectors();
    let pairs = v["pairs"].as_array().expect("pairs");
    assert!(pairs.len() >= 3000);
    for pair in pairs {
        let a = pair[0].as_str().unwrap();
        let b = pair[1].as_str().unwrap();
        let expected = match pair[2].as_i64().unwrap() {
            -1 => Ordering::Less,
            0 => Ordering::Equal,
            _ => Ordering::Greater,
        };
        assert_eq!(invariant_compare(a, b), expected, "{a:?} vs {b:?}");
    }
}

#[test]
fn the_dotnet_sort_is_in_order() {
    // .NET's List.Sort is unstable, so only the pairwise order is meaningful.
    let v = vectors();
    let sorted: Vec<&str> = v["sorted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(sorted.len() > 400);
    for w in sorted.windows(2) {
        assert_ne!(
            invariant_compare(w[0], w[1]),
            Ordering::Greater,
            "{:?} before {:?}",
            w[0],
            w[1]
        );
    }
}
