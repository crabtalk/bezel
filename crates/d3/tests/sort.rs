use d3::array;
use serde_json::Value;

/// Each array in `d3/sort.json` sorts as V8's `Array.prototype.sort` sorted
/// it, with a comparator that is not a total order as well as a stable one.
#[test]
fn sort_by_runs_as_v8_does() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("d3/sort.json")).expect("sort.json");
    let order = |n: i64| n.cmp(&0);
    for case in cases {
        let numbers = |key: &str| -> Vec<i64> {
            case[key]
                .as_array()
                .expect("array")
                .iter()
                .map(|v| v.as_i64().expect("integer"))
                .collect()
        };
        let mut items = numbers("input");
        match case["comparator"].as_str() {
            Some("inconsistent") => {
                array::sort_by(&mut items, |a, b| order((a * 31 + b * 17) % 7 - 3))
            }
            _ => array::sort_by(&mut items, |a, b| (a % 10).cmp(&(b % 10))),
        }
        assert_eq!(
            items,
            numbers("output"),
            "{} of {}",
            case["comparator"],
            items.len()
        );
    }
}
