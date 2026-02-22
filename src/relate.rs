use std::collections::{HashMap, HashSet};

use crate::model::{Cluster, Priority, RelateResult, Relationship, ScanResult, TodoItem};

const STOPWORDS: &[&str] = &[
    "a", "an", "the", "is", "it", "in", "to", "of", "for", "on", "and", "or", "but", "not", "with",
    "this", "that", "from", "be", "as", "at", "by", "do", "has", "have", "was", "were", "will",
    "can", "should", "would", "could", "may", "might", "need", "todo", "fixme", "hack", "xxx",
    "bug", "note",
];

const PROXIMITY_WEIGHT: f64 = 0.30;
const KEYWORD_WEIGHT: f64 = 0.35;
const CROSSREF_WEIGHT: f64 = 0.25;
const TAG_WEIGHT: f64 = 0.10;

pub fn extract_keywords(message: &str) -> HashSet<String> {
    let stopwords: HashSet<&str> = STOPWORDS.iter().copied().collect();
    message
        .split(|c: char| !c.is_alphanumeric() && c != '#')
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 1 && !stopwords.contains(w.as_str()))
        .collect()
}

pub fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    intersection / union
}

fn proximity_score(a: &TodoItem, b: &TodoItem, threshold: usize) -> f64 {
    if a.file != b.file {
        return 0.0;
    }
    let distance = a.line.abs_diff(b.line);
    if distance > threshold {
        return 0.0;
    }
    1.0 - (distance as f64 / threshold as f64)
}

fn cross_ref_score(a: &TodoItem, b: &TodoItem) -> f64 {
    let mut score = 0.0;
    if let (Some(ref ra), Some(ref rb)) = (&a.issue_ref, &b.issue_ref) {
        if ra == rb {
            score = 1.0;
        }
    }
    if let (Some(ref aa), Some(ref ab)) = (&a.author, &b.author) {
        if aa == ab {
            score = f64::max(score, 0.2);
        }
    }
    score
}

fn tag_score(a: &TodoItem, b: &TodoItem) -> f64 {
    if a.tag == b.tag {
        1.0
    } else {
        0.0
    }
}

struct ScoreComponents<'a> {
    prox: f64,
    kw_sim: f64,
    cross: f64,
    tag: f64,
    a: &'a TodoItem,
    b: &'a TodoItem,
    keywords_a: &'a HashSet<String>,
    keywords_b: &'a HashSet<String>,
}

fn build_reason(c: &ScoreComponents) -> String {
    let mut parts = Vec::new();
    if c.prox > 0.0 {
        parts.push("proximity".to_string());
    }
    if c.kw_sim > 0.0 {
        let shared: Vec<_> = c.keywords_a.intersection(c.keywords_b).cloned().collect();
        if !shared.is_empty() {
            parts.push(format!("shared_keyword:{}", shared.join(",")));
        }
    }
    if c.cross > 0.0 {
        if let (Some(ref ra), Some(ref rb)) = (&c.a.issue_ref, &c.b.issue_ref) {
            if ra == rb {
                parts.push(format!("same_issue:{}", ra));
            }
        }
        if let (Some(ref aa), Some(ref ab)) = (&c.a.author, &c.b.author) {
            if aa == ab {
                parts.push(format!("same_author:{}", aa));
            }
        }
    }
    if c.tag > 0.0 {
        parts.push(format!("same_tag:{}", c.a.tag));
    }
    parts.join(", ")
}

pub fn score_pair(
    a: &TodoItem,
    b: &TodoItem,
    proximity_threshold: usize,
    keywords_a: &HashSet<String>,
    keywords_b: &HashSet<String>,
) -> (f64, String) {
    let prox = proximity_score(a, b, proximity_threshold);
    let kw_sim = jaccard_similarity(keywords_a, keywords_b);
    let cross = cross_ref_score(a, b);
    let tag = tag_score(a, b);

    let score = (PROXIMITY_WEIGHT * prox
        + KEYWORD_WEIGHT * kw_sim
        + CROSSREF_WEIGHT * cross
        + TAG_WEIGHT * tag)
        .clamp(0.0, 1.0);

    let reason = build_reason(&ScoreComponents {
        prox,
        kw_sim,
        cross,
        tag,
        a,
        b,
        keywords_a,
        keywords_b,
    });
    (score, reason)
}

pub fn compute_relations(
    scan: &ScanResult,
    min_score: f64,
    proximity_threshold: usize,
) -> RelateResult {
    let items = &scan.items;
    let mut relationships = Vec::new();

    if items.len() < 2 {
        return RelateResult {
            relationships,
            clusters: None,
            total_relationships: 0,
            total_items: items.len(),
            min_score,
            target: None,
        };
    }

    // Pre-compute keywords
    let keywords: Vec<HashSet<String>> =
        items.iter().map(|i| extract_keywords(&i.message)).collect();

    for i in 0..items.len() {
        for j in (i + 1)..items.len() {
            let (score, reason) = score_pair(
                &items[i],
                &items[j],
                proximity_threshold,
                &keywords[i],
                &keywords[j],
            );
            if score >= min_score {
                relationships.push(Relationship {
                    from: format!("{}:{}", items[i].file, items[i].line),
                    to: format!("{}:{}", items[j].file, items[j].line),
                    score,
                    reason,
                });
            }
        }
    }

    let total_relationships = relationships.len();

    RelateResult {
        relationships,
        clusters: None,
        total_relationships,
        total_items: items.len(),
        min_score,
        target: None,
    }
}

pub fn filter_for_item(result: RelateResult, file: &str, line: usize) -> RelateResult {
    let target = format!("{}:{}", file, line);
    let filtered: Vec<Relationship> = result
        .relationships
        .into_iter()
        .filter(|r| r.from == target || r.to == target)
        .collect();
    let total_relationships = filtered.len();

    RelateResult {
        relationships: filtered,
        clusters: None,
        total_relationships,
        total_items: result.total_items,
        min_score: result.min_score,
        target: Some(target),
    }
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
    }
}

pub fn generate_theme(items: &[&TodoItem]) -> String {
    let mut freq: HashMap<String, usize> = HashMap::new();

    for item in items {
        for kw in extract_keywords(&item.message) {
            *freq.entry(kw).or_insert(0) += 1;
        }
        // Include directory names
        if let Some(dir) = std::path::Path::new(&item.file).parent() {
            let dir_str = dir.to_string_lossy().to_string();
            if !dir_str.is_empty() && dir_str != "." {
                for part in dir_str.split('/') {
                    if !part.is_empty() {
                        *freq.entry(part.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    let mut entries: Vec<_> = freq.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    entries
        .into_iter()
        .take(3)
        .map(|(k, _)| k)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn compute_suggested_order(items: &mut [&TodoItem]) {
    items.sort_by(|a, b| {
        let priority_ord = |p: &Priority| -> u8 {
            match p {
                Priority::Urgent => 0,
                Priority::High => 1,
                Priority::Normal => 2,
            }
        };
        priority_ord(&a.priority)
            .cmp(&priority_ord(&b.priority))
            .then(b.tag.severity().cmp(&a.tag.severity()))
            .then(a.file.cmp(&b.file))
            .then(a.line.cmp(&b.line))
    });
}

pub fn build_clusters(relationships: &[Relationship], items: &[TodoItem]) -> Vec<Cluster> {
    if items.is_empty() {
        return Vec::new();
    }

    // Build location -> index map
    let loc_to_idx: HashMap<String, usize> = items
        .iter()
        .enumerate()
        .map(|(i, item)| (format!("{}:{}", item.file, item.line), i))
        .collect();

    let mut uf = UnionFind::new(items.len());

    for rel in relationships {
        if let (Some(&i), Some(&j)) = (loc_to_idx.get(&rel.from), loc_to_idx.get(&rel.to)) {
            uf.union(i, j);
        }
    }

    // Group by root
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..items.len() {
        let root = uf.find(i);
        groups.entry(root).or_default().push(i);
    }

    // Filter to clusters with 2+ members and build
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut cluster_id = 1;

    let mut sorted_groups: Vec<_> = groups.into_iter().filter(|(_, v)| v.len() >= 2).collect();
    sorted_groups.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));

    for (_, member_indices) in sorted_groups {
        let mut member_items: Vec<&TodoItem> = member_indices.iter().map(|&i| &items[i]).collect();
        let theme = generate_theme(&member_items);

        compute_suggested_order(&mut member_items);

        let item_locs: Vec<String> = member_items
            .iter()
            .map(|i| format!("{}:{}", i.file, i.line))
            .collect();

        // Filter relationships to this cluster
        let cluster_locs: HashSet<&String> = item_locs.iter().collect();
        let cluster_rels: Vec<Relationship> = relationships
            .iter()
            .filter(|r| cluster_locs.contains(&r.from) && cluster_locs.contains(&r.to))
            .cloned()
            .collect();

        clusters.push(Cluster {
            id: cluster_id,
            theme,
            items: item_locs.clone(),
            suggested_order: item_locs,
            relationships: cluster_rels,
        });
        cluster_id += 1;
    }

    clusters
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Priority, Tag};

    fn make_item(file: &str, line: usize, tag: Tag, message: &str) -> TodoItem {
        TodoItem {
            file: file.to_string(),
            line,
            tag,
            message: message.to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        }
    }

    // --- extract_keywords ---

    #[test]
    fn extract_keywords_removes_stopwords_and_lowercases() {
        let kw = extract_keywords("Implement the input validation for users");
        assert!(kw.contains("implement"));
        assert!(kw.contains("input"));
        assert!(kw.contains("validation"));
        assert!(kw.contains("users"));
        assert!(!kw.contains("the"));
        assert!(!kw.contains("for"));
    }

    #[test]
    fn extract_keywords_empty_string() {
        let kw = extract_keywords("");
        assert!(kw.is_empty());
    }

    #[test]
    fn extract_keywords_single_char_words_filtered() {
        let kw = extract_keywords("a b c fix");
        assert!(kw.contains("fix"));
        assert!(!kw.contains("b"));
        assert!(!kw.contains("c"));
    }

    // --- jaccard_similarity ---

    #[test]
    fn jaccard_empty_sets() {
        let a: HashSet<String> = HashSet::new();
        let b: HashSet<String> = HashSet::new();
        assert_eq!(jaccard_similarity(&a, &b), 0.0);
    }

    #[test]
    fn jaccard_identical_sets() {
        let a: HashSet<String> = ["auth", "login"].iter().map(|s| s.to_string()).collect();
        let b = a.clone();
        assert_eq!(jaccard_similarity(&a, &b), 1.0);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let a: HashSet<String> = ["auth", "login", "user"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let b: HashSet<String> = ["auth", "token", "user"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // intersection: {auth, user} = 2, union: {auth, login, user, token} = 4
        let sim = jaccard_similarity(&a, &b);
        assert!((sim - 0.5).abs() < f64::EPSILON);
    }

    // --- score_pair ---

    #[test]
    fn score_pair_same_file_proximity_within_threshold() {
        let a = make_item("src/main.rs", 10, Tag::Todo, "fix auth");
        let b = make_item("src/main.rs", 15, Tag::Fixme, "broken auth");
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, reason) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        assert!(score > 0.0);
        assert!(reason.contains("proximity"));
    }

    #[test]
    fn score_pair_same_file_proximity_beyond_threshold() {
        let a = make_item("src/main.rs", 10, Tag::Todo, "alpha");
        let b = make_item("src/main.rs", 100, Tag::Fixme, "beta");
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, reason) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        // proximity=0, no shared keywords, no crossref, different tags
        assert_eq!(score, 0.0);
        assert!(reason.is_empty());
    }

    #[test]
    fn score_pair_different_files_no_proximity() {
        let a = make_item("src/auth.rs", 10, Tag::Todo, "alpha");
        let b = make_item("src/db.rs", 10, Tag::Fixme, "beta");
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, _) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn score_pair_shared_keywords() {
        let a = make_item(
            "src/auth.rs",
            10,
            Tag::Todo,
            "implement authentication validation",
        );
        let b = make_item("src/db.rs", 50, Tag::Fixme, "fix authentication check");
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, reason) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        assert!(score > 0.0);
        assert!(reason.contains("shared_keyword"));
        assert!(reason.contains("authentication"));
    }

    #[test]
    fn score_pair_same_issue_ref() {
        let mut a = make_item("src/auth.rs", 10, Tag::Todo, "alpha");
        let mut b = make_item("src/db.rs", 50, Tag::Fixme, "beta");
        a.issue_ref = Some("#42".to_string());
        b.issue_ref = Some("#42".to_string());
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, reason) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        assert!(score > 0.0);
        assert!(reason.contains("same_issue:#42"));
    }

    #[test]
    fn score_pair_same_author() {
        let mut a = make_item("src/auth.rs", 10, Tag::Todo, "alpha");
        let mut b = make_item("src/db.rs", 50, Tag::Fixme, "beta");
        a.author = Some("alice".to_string());
        b.author = Some("alice".to_string());
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, reason) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        assert!(score > 0.0);
        assert!(reason.contains("same_author:alice"));
    }

    #[test]
    fn score_pair_same_tag_bonus() {
        let a = make_item("src/auth.rs", 10, Tag::Todo, "alpha");
        let b = make_item("src/db.rs", 50, Tag::Todo, "beta");
        let kw_a = extract_keywords(&a.message);
        let kw_b = extract_keywords(&b.message);
        let (score, reason) = score_pair(&a, &b, 10, &kw_a, &kw_b);
        assert!(score > 0.0);
        assert!(reason.contains("same_tag:TODO"));
    }

    // --- compute_relations ---

    #[test]
    fn compute_relations_empty_input() {
        let scan = ScanResult {
            items: vec![],
            files_scanned: 0,
        };
        let result = compute_relations(&scan, 0.3, 10);
        assert!(result.relationships.is_empty());
        assert_eq!(result.total_items, 0);
    }

    #[test]
    fn compute_relations_single_item() {
        let scan = ScanResult {
            items: vec![make_item("src/main.rs", 10, Tag::Todo, "fix something")],
            files_scanned: 1,
        };
        let result = compute_relations(&scan, 0.3, 10);
        assert!(result.relationships.is_empty());
        assert_eq!(result.total_items, 1);
    }

    #[test]
    fn compute_relations_filters_by_min_score() {
        let scan = ScanResult {
            items: vec![
                make_item("src/main.rs", 10, Tag::Todo, "fix authentication"),
                make_item("src/main.rs", 12, Tag::Fixme, "broken authentication"),
            ],
            files_scanned: 1,
        };
        // With min_score=0.0, should find relationship
        let result_low = compute_relations(&scan, 0.0, 10);
        assert!(!result_low.relationships.is_empty());

        // With min_score=1.0, should not find relationship (max score < 1.0 unless identical)
        let result_high = compute_relations(&scan, 1.0, 10);
        assert!(result_high.relationships.is_empty());
    }

    // --- filter_for_item ---

    #[test]
    fn filter_for_item_filters_correctly() {
        let result = RelateResult {
            relationships: vec![
                Relationship {
                    from: "src/a.rs:10".to_string(),
                    to: "src/b.rs:20".to_string(),
                    score: 0.5,
                    reason: "proximity".to_string(),
                },
                Relationship {
                    from: "src/c.rs:30".to_string(),
                    to: "src/d.rs:40".to_string(),
                    score: 0.5,
                    reason: "keyword".to_string(),
                },
                Relationship {
                    from: "src/b.rs:20".to_string(),
                    to: "src/e.rs:50".to_string(),
                    score: 0.4,
                    reason: "tag".to_string(),
                },
            ],
            clusters: None,
            total_relationships: 3,
            total_items: 5,
            min_score: 0.3,
            target: None,
        };

        let filtered = filter_for_item(result, "src/b.rs", 20);
        assert_eq!(filtered.relationships.len(), 2);
        assert_eq!(filtered.target, Some("src/b.rs:20".to_string()));
    }

    // --- build_clusters ---

    #[test]
    fn build_clusters_connected_components() {
        let items = vec![
            make_item("src/a.rs", 10, Tag::Todo, "fix auth"),
            make_item("src/a.rs", 12, Tag::Fixme, "broken auth"),
            make_item("src/b.rs", 100, Tag::Note, "unrelated item"),
        ];
        let relationships = vec![Relationship {
            from: "src/a.rs:10".to_string(),
            to: "src/a.rs:12".to_string(),
            score: 0.5,
            reason: "proximity".to_string(),
        }];

        let clusters = build_clusters(&relationships, &items);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].items.len(), 2);
        assert!(clusters[0].items.contains(&"src/a.rs:10".to_string()));
        assert!(clusters[0].items.contains(&"src/a.rs:12".to_string()));
    }

    #[test]
    fn build_clusters_disjoint_groups() {
        let items = vec![
            make_item("src/a.rs", 10, Tag::Todo, "fix auth"),
            make_item("src/a.rs", 12, Tag::Fixme, "broken auth"),
            make_item("src/b.rs", 20, Tag::Todo, "fix database"),
            make_item("src/b.rs", 22, Tag::Bug, "db crash"),
        ];
        let relationships = vec![
            Relationship {
                from: "src/a.rs:10".to_string(),
                to: "src/a.rs:12".to_string(),
                score: 0.5,
                reason: "proximity".to_string(),
            },
            Relationship {
                from: "src/b.rs:20".to_string(),
                to: "src/b.rs:22".to_string(),
                score: 0.5,
                reason: "proximity".to_string(),
            },
        ];

        let clusters = build_clusters(&relationships, &items);
        assert_eq!(clusters.len(), 2);
    }

    // --- generate_theme ---

    #[test]
    fn generate_theme_extracts_top_keywords() {
        let items = [
            make_item(
                "src/auth.rs",
                10,
                Tag::Todo,
                "fix authentication validation",
            ),
            make_item("src/auth.rs", 20, Tag::Fixme, "broken authentication check"),
        ];
        let refs: Vec<&TodoItem> = items.iter().collect();
        let theme = generate_theme(&refs);
        assert!(theme.contains("authentication"));
    }

    // --- compute_suggested_order ---

    #[test]
    fn compute_suggested_order_by_priority_then_severity() {
        let items = [
            make_item("src/a.rs", 10, Tag::Note, "low prio note"),
            {
                let mut item = make_item("src/b.rs", 20, Tag::Bug, "urgent bug");
                item.priority = Priority::Urgent;
                item
            },
            {
                let mut item = make_item("src/c.rs", 30, Tag::Todo, "high prio todo");
                item.priority = Priority::High;
                item
            },
        ];
        let mut refs: Vec<&TodoItem> = items.iter().collect();
        compute_suggested_order(&mut refs);

        assert_eq!(refs[0].priority, Priority::Urgent);
        assert_eq!(refs[1].priority, Priority::High);
        assert_eq!(refs[2].priority, Priority::Normal);
    }
}
