use crate::model::*;

fn build_sarif_envelope(results: Vec<serde_json::Value>, rules: Vec<serde_json::Value>) -> String {
    let sarif = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "todox",
                    "version": env!("CARGO_PKG_VERSION"),
                    "rules": rules
                }
            },
            "results": results
        }]
    });
    serde_json::to_string_pretty(&sarif).expect("failed to serialize SARIF")
}

fn rule_id(tag: &Tag) -> String {
    format!("todox/{}", tag.as_str())
}

fn collect_rules(items: &[&TodoItem]) -> Vec<serde_json::Value> {
    let mut seen = std::collections::BTreeSet::new();
    let mut rules = Vec::new();
    for item in items {
        let id = rule_id(&item.tag);
        if seen.insert(id.clone()) {
            rules.push(serde_json::json!({
                "id": id,
                "shortDescription": {
                    "text": format!("{} comment", item.tag.as_str())
                }
            }));
        }
    }
    rules
}

fn item_to_result(item: &TodoItem) -> serde_json::Value {
    let severity = Severity::from_item(item);
    serde_json::json!({
        "ruleId": rule_id(&item.tag),
        "level": severity.as_sarif_level(),
        "message": {
            "text": item.message
        },
        "locations": [{
            "physicalLocation": {
                "artifactLocation": {
                    "uri": item.file
                },
                "region": {
                    "startLine": item.line
                }
            }
        }]
    })
}

pub fn format_list(result: &ScanResult) -> String {
    let results: Vec<serde_json::Value> = result.items.iter().map(item_to_result).collect();
    let all_items: Vec<&TodoItem> = result.items.iter().collect();
    let rules = collect_rules(&all_items);
    let mut output = build_sarif_envelope(results, rules);
    output.push('\n');
    output
}

pub fn format_diff(result: &DiffResult) -> String {
    let results: Vec<serde_json::Value> = result
        .entries
        .iter()
        .map(|entry| {
            let mut r = item_to_result(&entry.item);
            let status = match entry.status {
                DiffStatus::Added => "added",
                DiffStatus::Removed => "removed",
            };
            r.as_object_mut().unwrap().insert(
                "properties".to_string(),
                serde_json::json!({ "diffStatus": status }),
            );
            r
        })
        .collect();

    let all_items: Vec<&TodoItem> = result.entries.iter().map(|e| &e.item).collect();
    let rules = collect_rules(&all_items);
    let mut output = build_sarif_envelope(results, rules);
    output.push('\n');
    output
}

pub fn format_check(result: &CheckResult) -> String {
    let results: Vec<serde_json::Value> = result
        .violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "ruleId": format!("todox/check/{}", v.rule),
                "level": if result.passed { "note" } else { "error" },
                "message": {
                    "text": v.message
                }
            })
        })
        .collect();

    let rules: Vec<serde_json::Value> = result
        .violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "id": format!("todox/check/{}", v.rule),
                "shortDescription": {
                    "text": format!("{} check", v.rule)
                }
            })
        })
        .collect();

    // If passed with no violations, add a summary result
    let final_results = if result.passed && results.is_empty() {
        vec![serde_json::json!({
            "ruleId": "todox/check/summary",
            "level": "note",
            "message": {
                "text": format!("All checks passed ({} items total)", result.total)
            }
        })]
    } else {
        results
    };

    let final_rules = if result.passed && rules.is_empty() {
        vec![serde_json::json!({
            "id": "todox/check/summary",
            "shortDescription": {
                "text": "todox check summary"
            }
        })]
    } else {
        rules
    };

    let mut output = build_sarif_envelope(final_results, final_rules);
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_item(tag: Tag, message: &str) -> TodoItem {
        TodoItem {
            file: "src/main.rs".to_string(),
            line: 10,
            tag,
            message: message.to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
        }
    }

    #[test]
    fn test_format_list_sarif_structure() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "implement feature")],
            files_scanned: 1,
        };
        let output = format_list(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(sarif["version"], "2.1.0");
        assert_eq!(sarif["runs"][0]["tool"]["driver"]["name"], "todox");

        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["ruleId"], "todox/TODO");
        assert_eq!(results[0]["level"], "warning");
        assert_eq!(results[0]["message"]["text"], "implement feature");
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/main.rs"
        );
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["region"]["startLine"],
            10
        );
    }

    #[test]
    fn test_format_list_sarif_severity() {
        let result = ScanResult {
            items: vec![
                sample_item(Tag::Bug, "critical"),
                sample_item(Tag::Note, "info"),
            ],
            files_scanned: 1,
        };
        let output = format_list(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["level"], "error");
        assert_eq!(results[1]["level"], "note");
    }

    #[test]
    fn test_format_list_sarif_rules_deduplication() {
        let result = ScanResult {
            items: vec![
                sample_item(Tag::Todo, "first"),
                sample_item(Tag::Todo, "second"),
                sample_item(Tag::Bug, "a bug"),
            ],
            files_scanned: 1,
        };
        let output = format_list(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(rules.len(), 2); // TODO and BUG, not 3
    }

    #[test]
    fn test_format_diff_sarif_has_diff_status() {
        let result = DiffResult {
            entries: vec![DiffEntry {
                status: DiffStatus::Added,
                item: sample_item(Tag::Fixme, "new fix"),
            }],
            added_count: 1,
            removed_count: 0,
            base_ref: "main".to_string(),
        };
        let output = format_diff(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["properties"]["diffStatus"], "added");
    }

    #[test]
    fn test_format_check_sarif_pass() {
        let result = CheckResult {
            passed: true,
            total: 5,
            violations: vec![],
        };
        let output = format_check(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["level"], "note");
        assert!(results[0]["message"]["text"]
            .as_str()
            .unwrap()
            .contains("passed"));
    }

    #[test]
    fn test_format_check_sarif_fail() {
        let result = CheckResult {
            passed: false,
            total: 10,
            violations: vec![CheckViolation {
                rule: "max".to_string(),
                message: "10 exceeds max 5".to_string(),
            }],
        };
        let output = format_check(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["ruleId"], "todox/check/max");
        assert_eq!(results[0]["level"], "error");
    }
}
