use crate::model::*;

fn build_sarif_envelope(results: Vec<serde_json::Value>, rules: Vec<serde_json::Value>) -> String {
    let sarif = serde_json::json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "todo-scan",
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
    format!("todo-scan/{}", tag.as_str())
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
    let mut result = serde_json::json!({
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
    });
    if let Some(ref deadline) = item.deadline {
        result
            .as_object_mut()
            .expect("SARIF result should be a JSON object")
            .insert(
                "properties".to_string(),
                serde_json::json!({ "deadline": deadline.to_string() }),
            );
    }
    result
}

pub fn format_list(result: &ScanResult) -> String {
    let results: Vec<serde_json::Value> = result.items.iter().map(item_to_result).collect();
    let all_items: Vec<&TodoItem> = result.items.iter().collect();
    let rules = collect_rules(&all_items);
    let mut output = build_sarif_envelope(results, rules);
    output.push('\n');
    output
}

pub fn format_search(result: &SearchResult) -> String {
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
            r.as_object_mut()
                .expect("SARIF result should be a JSON object")
                .insert(
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

pub fn format_blame(result: &BlameResult) -> String {
    let results: Vec<serde_json::Value> = result
        .entries
        .iter()
        .map(|entry| {
            let mut r = item_to_result(&entry.item);
            r.as_object_mut()
                .expect("SARIF result should be a JSON object")
                .insert(
                    "properties".to_string(),
                    serde_json::json!({
                        "blame": {
                            "author": entry.blame.author,
                            "email": entry.blame.email,
                            "date": entry.blame.date,
                            "ageDays": entry.blame.age_days,
                            "commit": entry.blame.commit,
                            "stale": entry.stale,
                        }
                    }),
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

pub fn format_lint(result: &LintResult) -> String {
    let results: Vec<serde_json::Value> = result
        .violations
        .iter()
        .map(|v| {
            let mut r = serde_json::json!({
                "ruleId": format!("todo-scan/lint/{}", v.rule),
                "level": "error",
                "message": {
                    "text": v.message
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": v.file
                        },
                        "region": {
                            "startLine": v.line
                        }
                    }
                }]
            });
            if let Some(ref suggestion) = v.suggestion {
                r.as_object_mut()
                    .expect("SARIF result should be a JSON object")
                    .insert(
                        "fixes".to_string(),
                        serde_json::json!([{
                            "description": {
                                "text": suggestion
                            }
                        }]),
                    );
            }
            r
        })
        .collect();

    let mut seen = std::collections::BTreeSet::new();
    let rules: Vec<serde_json::Value> = result
        .violations
        .iter()
        .filter_map(|v| {
            let id = format!("todo-scan/lint/{}", v.rule);
            if seen.insert(id.clone()) {
                Some(serde_json::json!({
                    "id": id,
                    "shortDescription": {
                        "text": format!("{} lint rule", v.rule)
                    }
                }))
            } else {
                None
            }
        })
        .collect();

    let final_results = if result.passed && results.is_empty() {
        vec![serde_json::json!({
            "ruleId": "todo-scan/lint/summary",
            "level": "note",
            "message": {
                "text": format!("All lint checks passed ({} items)", result.total_items)
            }
        })]
    } else {
        results
    };

    let final_rules = if result.passed && rules.is_empty() {
        vec![serde_json::json!({
            "id": "todo-scan/lint/summary",
            "shortDescription": {
                "text": "todo-scan lint summary"
            }
        })]
    } else {
        rules
    };

    let mut output = build_sarif_envelope(final_results, final_rules);
    output.push('\n');
    output
}

pub fn format_check(result: &CheckResult) -> String {
    let results: Vec<serde_json::Value> = result
        .violations
        .iter()
        .map(|v| {
            serde_json::json!({
                "ruleId": format!("todo-scan/check/{}", v.rule),
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
                "id": format!("todo-scan/check/{}", v.rule),
                "shortDescription": {
                    "text": format!("{} check", v.rule)
                }
            })
        })
        .collect();

    // If passed with no violations, add a summary result
    let final_results = if result.passed && results.is_empty() {
        vec![serde_json::json!({
            "ruleId": "todo-scan/check/summary",
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
            "id": "todo-scan/check/summary",
            "shortDescription": {
                "text": "todo-scan check summary"
            }
        })]
    } else {
        rules
    };

    let mut output = build_sarif_envelope(final_results, final_rules);
    output.push('\n');
    output
}

pub fn format_clean(result: &CleanResult) -> String {
    let results: Vec<serde_json::Value> = result
        .violations
        .iter()
        .map(|v| {
            let mut r = serde_json::json!({
                "ruleId": format!("todo-scan/clean/{}", v.rule),
                "level": "error",
                "message": {
                    "text": v.message
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": v.file
                        },
                        "region": {
                            "startLine": v.line
                        }
                    }
                }]
            });
            let mut props = serde_json::Map::new();
            if let Some(ref issue_ref) = v.issue_ref {
                props.insert(
                    "issueRef".to_string(),
                    serde_json::Value::String(issue_ref.clone()),
                );
            }
            if let Some(ref duplicate_of) = v.duplicate_of {
                props.insert(
                    "duplicateOf".to_string(),
                    serde_json::Value::String(duplicate_of.clone()),
                );
            }
            if !props.is_empty() {
                r.as_object_mut()
                    .unwrap()
                    .insert("properties".to_string(), serde_json::Value::Object(props));
            }
            r
        })
        .collect();

    let mut seen = std::collections::BTreeSet::new();
    let rules: Vec<serde_json::Value> = result
        .violations
        .iter()
        .filter_map(|v| {
            let id = format!("todo-scan/clean/{}", v.rule);
            if seen.insert(id.clone()) {
                Some(serde_json::json!({
                    "id": id,
                    "shortDescription": {
                        "text": format!("{} clean rule", v.rule)
                    }
                }))
            } else {
                None
            }
        })
        .collect();

    let final_results = if result.passed && results.is_empty() {
        vec![serde_json::json!({
            "ruleId": "todo-scan/clean/summary",
            "level": "note",
            "message": {
                "text": format!("All clean checks passed ({} items)", result.total_items)
            }
        })]
    } else {
        results
    };

    let final_rules = if result.passed && rules.is_empty() {
        vec![serde_json::json!({
            "id": "todo-scan/clean/summary",
            "shortDescription": {
                "text": "todo-scan clean summary"
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
            deadline: None,
        }
    }

    #[test]
    fn test_format_list_sarif_structure() {
        let result = ScanResult {
            items: vec![sample_item(Tag::Todo, "implement feature")],
            files_scanned: 1,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert_eq!(sarif["version"], "2.1.0");
        assert_eq!(sarif["runs"][0]["tool"]["driver"]["name"], "todo-scan");

        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["ruleId"], "todo-scan/TODO");
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
            ignored_items: vec![],
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
            ignored_items: vec![],
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
        assert_eq!(results[0]["ruleId"], "todo-scan/check/max");
        assert_eq!(results[0]["level"], "error");
    }

    #[test]
    fn test_format_search_sarif() {
        let result = SearchResult {
            query: "fix".to_string(),
            exact: false,
            items: vec![sample_item(Tag::Fixme, "fix this")],
            match_count: 1,
            file_count: 1,
        };
        let output = format_search(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["ruleId"], "todo-scan/FIXME");
        assert_eq!(results[0]["level"], "error");
    }

    #[test]
    fn test_format_blame_sarif() {
        let result = BlameResult {
            entries: vec![BlameEntry {
                item: sample_item(Tag::Todo, "old item"),
                blame: BlameInfo {
                    author: "alice".to_string(),
                    email: "alice@test.com".to_string(),
                    date: "2024-01-01".to_string(),
                    age_days: 400,
                    commit: "abc123".to_string(),
                },
                stale: true,
            }],
            total: 1,
            avg_age_days: 400,
            stale_count: 1,
            stale_threshold_days: 365,
        };
        let output = format_blame(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        let props = &results[0]["properties"]["blame"];
        assert_eq!(props["author"], "alice");
        assert_eq!(props["stale"], true);
        assert_eq!(props["ageDays"], 400);
    }

    #[test]
    fn test_format_lint_sarif_pass() {
        let result = LintResult {
            passed: true,
            total_items: 5,
            violation_count: 0,
            violations: vec![],
        };
        let output = format_lint(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["level"], "note");
        assert_eq!(results[0]["ruleId"], "todo-scan/lint/summary");
    }

    #[test]
    fn test_format_lint_sarif_fail_with_suggestion() {
        let result = LintResult {
            passed: false,
            total_items: 1,
            violation_count: 1,
            violations: vec![LintViolation {
                file: "test.rs".to_string(),
                line: 5,
                rule: "no_bare_tags".to_string(),
                message: "bare tag".to_string(),
                suggestion: Some("add a message".to_string()),
            }],
        };
        let output = format_lint(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["ruleId"], "todo-scan/lint/no_bare_tags");
        assert!(results[0]["fixes"].is_array());
        assert_eq!(
            results[0]["fixes"][0]["description"]["text"],
            "add a message"
        );
    }

    #[test]
    fn test_format_lint_sarif_fail_without_suggestion() {
        let result = LintResult {
            passed: false,
            total_items: 1,
            violation_count: 1,
            violations: vec![LintViolation {
                file: "test.rs".to_string(),
                line: 5,
                rule: "uppercase_tag".to_string(),
                message: "tag not uppercase".to_string(),
                suggestion: None,
            }],
        };
        let output = format_lint(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert!(results[0].get("fixes").is_none());
    }

    #[test]
    fn test_format_clean_sarif_pass() {
        let result = CleanResult {
            passed: true,
            total_items: 3,
            stale_count: 0,
            duplicate_count: 0,
            violations: vec![],
        };
        let output = format_clean(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["ruleId"], "todo-scan/clean/summary");
        assert_eq!(results[0]["level"], "note");
    }

    #[test]
    fn test_format_clean_sarif_fail_with_duplicate() {
        let result = CleanResult {
            passed: false,
            total_items: 2,
            stale_count: 0,
            duplicate_count: 1,
            violations: vec![CleanViolation {
                file: "test.rs".to_string(),
                line: 10,
                rule: "duplicate".to_string(),
                message: "duplicate TODO".to_string(),
                issue_ref: None,
                duplicate_of: Some("test.rs:5".to_string()),
            }],
        };
        let output = format_clean(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["properties"]["duplicateOf"], "test.rs:5");
    }

    #[test]
    fn test_format_clean_sarif_fail_with_issue_ref() {
        let result = CleanResult {
            passed: false,
            total_items: 1,
            stale_count: 1,
            duplicate_count: 0,
            violations: vec![CleanViolation {
                file: "test.rs".to_string(),
                line: 10,
                rule: "stale_issue".to_string(),
                message: "stale issue".to_string(),
                issue_ref: Some("#42".to_string()),
                duplicate_of: None,
            }],
        };
        let output = format_clean(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["properties"]["issueRef"], "#42");
    }

    #[test]
    fn test_format_clean_sarif_fail_no_properties() {
        let result = CleanResult {
            passed: false,
            total_items: 1,
            stale_count: 0,
            duplicate_count: 0,
            violations: vec![CleanViolation {
                file: "test.rs".to_string(),
                line: 10,
                rule: "some_rule".to_string(),
                message: "violation".to_string(),
                issue_ref: None,
                duplicate_of: None,
            }],
        };
        let output = format_clean(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        // No properties should be added when both issue_ref and duplicate_of are None
        assert!(results[0].get("properties").is_none());
    }

    #[test]
    fn test_format_diff_sarif_removed() {
        let result = DiffResult {
            entries: vec![DiffEntry {
                status: DiffStatus::Removed,
                item: sample_item(Tag::Todo, "removed task"),
            }],
            added_count: 0,
            removed_count: 1,
            base_ref: "main".to_string(),
        };
        let output = format_diff(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results[0]["properties"]["diffStatus"], "removed");
    }

    #[test]
    fn test_item_to_result_with_deadline() {
        use crate::deadline::Deadline;
        let item = TodoItem {
            file: "test.rs".to_string(),
            line: 1,
            tag: Tag::Todo,
            message: "task".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: Some(Deadline {
                year: 2025,
                month: 6,
                day: 1,
            }),
        };
        let result = item_to_result(&item);
        assert!(result["properties"]["deadline"].as_str().is_some());
    }

    #[test]
    fn test_format_list_sarif_empty() {
        let result = ScanResult {
            items: vec![],
            files_scanned: 0,
            ignored_items: vec![],
        };
        let output = format_list(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let results = sarif["runs"][0]["results"].as_array().unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_format_lint_sarif_deduplicates_rules() {
        let result = LintResult {
            passed: false,
            total_items: 2,
            violation_count: 2,
            violations: vec![
                LintViolation {
                    file: "a.rs".to_string(),
                    line: 1,
                    rule: "no_bare_tags".to_string(),
                    message: "first".to_string(),
                    suggestion: None,
                },
                LintViolation {
                    file: "b.rs".to_string(),
                    line: 2,
                    rule: "no_bare_tags".to_string(),
                    message: "second".to_string(),
                    suggestion: None,
                },
            ],
        };
        let output = format_lint(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_format_clean_sarif_deduplicates_rules() {
        let result = CleanResult {
            passed: false,
            total_items: 2,
            stale_count: 2,
            duplicate_count: 0,
            violations: vec![
                CleanViolation {
                    file: "a.rs".to_string(),
                    line: 1,
                    rule: "stale".to_string(),
                    message: "first".to_string(),
                    issue_ref: None,
                    duplicate_of: None,
                },
                CleanViolation {
                    file: "b.rs".to_string(),
                    line: 2,
                    rule: "stale".to_string(),
                    message: "second".to_string(),
                    issue_ref: None,
                    duplicate_of: None,
                },
            ],
        };
        let output = format_clean(&result);
        let sarif: serde_json::Value = serde_json::from_str(&output).unwrap();
        let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap();
        assert_eq!(rules.len(), 1);
    }
}
