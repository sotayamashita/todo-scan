use crate::model::ReportResult;

/// Render a self-contained HTML dashboard report.
pub fn render_html(report: &ReportResult) -> String {
    let json_data = serde_json::to_string(report).expect("failed to serialize report");
    // Escape all `<` in JSON data to prevent breaking the HTML script block.
    // HTML5 parsers match </script> case-insensitively, so we must neutralize
    // every `<` rather than just the lowercase variant.
    let safe_json = json_data.replace('<', "\\u003c");

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>todo-scan Technical Debt Report</title>
<style>
:root {{
  --bg: #ffffff;
  --bg-card: #f8f9fa;
  --bg-table-head: #e9ecef;
  --text: #212529;
  --text-muted: #6c757d;
  --border: #dee2e6;
  --accent: #0d6efd;
  --danger: #dc3545;
  --warning: #ffc107;
  --success: #198754;
  --info: #0dcaf0;
}}
@media (prefers-color-scheme: dark) {{
  :root {{
    --bg: #1a1a2e;
    --bg-card: #16213e;
    --bg-table-head: #0f3460;
    --text: #e0e0e0;
    --text-muted: #a0a0a0;
    --border: #2a2a4a;
    --accent: #4dabf7;
    --danger: #ff6b6b;
    --warning: #ffd43b;
    --success: #51cf66;
    --info: #66d9e8;
  }}
}}
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: var(--bg);
  color: var(--text);
  line-height: 1.6;
  padding: 2rem;
  max-width: 1200px;
  margin: 0 auto;
}}
h1 {{ margin-bottom: 0.25rem; }}
.subtitle {{ color: var(--text-muted); margin-bottom: 2rem; font-size: 0.9rem; }}
.cards {{
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
  gap: 1rem;
  margin-bottom: 2rem;
}}
.card {{
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 1.25rem;
  text-align: center;
}}
.card .value {{
  font-size: 2rem;
  font-weight: 700;
  line-height: 1.2;
}}
.card .label {{
  font-size: 0.8rem;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}}
.card.danger .value {{ color: var(--danger); }}
.card.warning .value {{ color: var(--warning); }}
.card.success .value {{ color: var(--success); }}
.section {{
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 1.5rem;
  margin-bottom: 1.5rem;
}}
.section h2 {{
  font-size: 1.1rem;
  margin-bottom: 1rem;
  border-bottom: 1px solid var(--border);
  padding-bottom: 0.5rem;
}}
.chart-row {{
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1.5rem;
  margin-bottom: 1.5rem;
}}
canvas {{ width: 100% !important; height: 200px !important; }}
table {{
  width: 100%;
  border-collapse: collapse;
  font-size: 0.875rem;
}}
th, td {{
  text-align: left;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--border);
}}
th {{
  background: var(--bg-table-head);
  cursor: pointer;
  user-select: none;
  white-space: nowrap;
}}
th:hover {{ opacity: 0.8; }}
tr:hover td {{ background: var(--bg-table-head); }}
.tag {{ font-weight: 600; }}
.tag-TODO {{ color: var(--warning); }}
.tag-FIXME, .tag-BUG {{ color: var(--danger); }}
.tag-HACK, .tag-XXX {{ color: #e67700; }}
.tag-NOTE {{ color: var(--info); }}
.priority-urgent {{ color: var(--danger); font-weight: 700; }}
.priority-high {{ color: #e67700; font-weight: 600; }}
.bar-container {{
  display: flex;
  align-items: center;
  gap: 0.5rem;
}}
.bar {{
  height: 18px;
  border-radius: 3px;
  background: var(--accent);
  min-width: 2px;
}}
footer {{
  text-align: center;
  color: var(--text-muted);
  font-size: 0.8rem;
  margin-top: 2rem;
  padding-top: 1rem;
  border-top: 1px solid var(--border);
}}
</style>
</head>
<body>
<h1>todo-scan Technical Debt Report</h1>
<p class="subtitle">Generated: <span id="generated-at"></span></p>

<div class="cards" id="summary-cards"></div>

<div class="chart-row">
  <div class="section">
    <h2>Trend</h2>
    <canvas id="chart-trend"></canvas>
    <p id="trend-empty" style="display:none;color:var(--text-muted);text-align:center;padding:2rem;">No history data available</p>
  </div>
  <div class="section">
    <h2>Age Distribution</h2>
    <canvas id="chart-age"></canvas>
  </div>
</div>

<div class="chart-row">
  <div class="section">
    <h2>Tags</h2>
    <canvas id="chart-tags"></canvas>
  </div>
  <div class="section">
    <h2>Priority</h2>
    <canvas id="chart-priority"></canvas>
  </div>
</div>

<div class="chart-row">
  <div class="section">
    <h2>Top Authors</h2>
    <div id="authors-list"></div>
  </div>
  <div class="section">
    <h2>Hotspot Files</h2>
    <div id="hotspots-list"></div>
  </div>
</div>

<div class="section">
  <h2>All Items (<span id="item-count"></span>)</h2>
  <table id="items-table">
    <thead>
      <tr>
        <th data-col="file">File</th>
        <th data-col="line">Line</th>
        <th data-col="tag">Tag</th>
        <th data-col="priority">Priority</th>
        <th data-col="message">Message</th>
        <th data-col="author">Author</th>
      </tr>
    </thead>
    <tbody></tbody>
  </table>
</div>

<footer>Generated by <strong>todo-scan</strong></footer>

<script>
const REPORT_DATA = {safe_json};

(function() {{
  const D = REPORT_DATA;

  // Summary cards
  document.getElementById('generated-at').textContent = D.generated_at;
  document.getElementById('item-count').textContent = D.items.length;

  const cards = [
    {{ value: D.summary.total_items, label: 'Total Items', cls: '' }},
    {{ value: D.summary.total_files, label: 'Files with TODOs', cls: '' }},
    {{ value: D.summary.files_scanned, label: 'Files Scanned', cls: '' }},
    {{ value: D.summary.urgent_count, label: 'Urgent', cls: D.summary.urgent_count > 0 ? 'danger' : 'success' }},
    {{ value: D.summary.high_count, label: 'High Priority', cls: D.summary.high_count > 0 ? 'warning' : 'success' }},
    {{ value: D.summary.stale_count, label: 'Stale', cls: D.summary.stale_count > 0 ? 'warning' : 'success' }},
    {{ value: D.summary.avg_age_days + 'd', label: 'Avg Age', cls: '' }},
  ];
  const cardsEl = document.getElementById('summary-cards');
  cards.forEach(c => {{
    const div = document.createElement('div');
    div.className = 'card ' + c.cls;
    div.innerHTML = '<div class="value">' + c.value + '</div><div class="label">' + c.label + '</div>';
    cardsEl.appendChild(div);
  }});

  // Canvas bar chart helper
  function drawBarChart(canvasId, labels, values, colors) {{
    const canvas = document.getElementById(canvasId);
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
    const w = rect.width, h = rect.height;
    const max = Math.max(...values, 1);
    const barWidth = Math.min(40, (w - 60) / labels.length - 8);
    const startX = 50;
    const bottomY = h - 30;
    const chartH = bottomY - 10;

    // Grid lines
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--border');
    ctx.lineWidth = 0.5;
    for (let i = 0; i <= 4; i++) {{
      const y = bottomY - (chartH * i / 4);
      ctx.beginPath();
      ctx.moveTo(startX, y);
      ctx.lineTo(w, y);
      ctx.stroke();
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text-muted');
      ctx.font = '11px sans-serif';
      ctx.textAlign = 'right';
      ctx.fillText(Math.round(max * i / 4), startX - 5, y + 4);
    }}

    // Bars
    labels.forEach((label, i) => {{
      const x = startX + i * ((w - startX - 10) / labels.length) + ((w - startX - 10) / labels.length - barWidth) / 2;
      const barH = (values[i] / max) * chartH;
      ctx.fillStyle = colors[i % colors.length];
      ctx.fillRect(x, bottomY - barH, barWidth, barH);

      // Value on top
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text');
      ctx.font = '11px sans-serif';
      ctx.textAlign = 'center';
      if (values[i] > 0) ctx.fillText(values[i], x + barWidth / 2, bottomY - barH - 4);

      // Label below
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text-muted');
      ctx.font = '10px sans-serif';
      ctx.save();
      ctx.translate(x + barWidth / 2, bottomY + 5);
      ctx.rotate(labels.length > 6 ? Math.PI / 4 : 0);
      ctx.textAlign = labels.length > 6 ? 'left' : 'center';
      ctx.fillText(label, 0, 10);
      ctx.restore();
    }});
  }}

  // Line chart helper
  function drawLineChart(canvasId, labels, values) {{
    const canvas = document.getElementById(canvasId);
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
    const w = rect.width, h = rect.height;
    const max = Math.max(...values, 1);
    const startX = 50, bottomY = h - 30, chartH = bottomY - 10;

    // Grid
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--border');
    ctx.lineWidth = 0.5;
    for (let i = 0; i <= 4; i++) {{
      const y = bottomY - (chartH * i / 4);
      ctx.beginPath();
      ctx.moveTo(startX, y);
      ctx.lineTo(w, y);
      ctx.stroke();
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text-muted');
      ctx.font = '11px sans-serif';
      ctx.textAlign = 'right';
      ctx.fillText(Math.round(max * i / 4), startX - 5, y + 4);
    }}

    if (values.length < 2) return;

    // Line
    const stepX = (w - startX - 20) / (values.length - 1);
    ctx.strokeStyle = getComputedStyle(document.documentElement).getPropertyValue('--accent');
    ctx.lineWidth = 2;
    ctx.beginPath();
    values.forEach((v, i) => {{
      const x = startX + i * stepX;
      const y = bottomY - (v / max) * chartH;
      if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
    }});
    ctx.stroke();

    // Points and labels
    values.forEach((v, i) => {{
      const x = startX + i * stepX;
      const y = bottomY - (v / max) * chartH;
      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--accent');
      ctx.beginPath();
      ctx.arc(x, y, 3, 0, Math.PI * 2);
      ctx.fill();

      ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue('--text-muted');
      ctx.font = '10px sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(labels[i], x, bottomY + 14);
    }});
  }}

  // Trend chart
  if (D.history.length > 0) {{
    drawLineChart('chart-trend', D.history.map(h => h.date), D.history.map(h => h.count));
  }} else {{
    document.getElementById('chart-trend').style.display = 'none';
    document.getElementById('trend-empty').style.display = 'block';
  }}

  // Age histogram
  drawBarChart(
    'chart-age',
    D.age_histogram.map(b => b.label),
    D.age_histogram.map(b => b.count),
    ['#51cf66', '#94d82d', '#ffd43b', '#ff922b', '#ff6b6b', '#c92a2a']
  );

  // Tags chart
  const tagColors = {{
    'TODO': '#ffc107', 'FIXME': '#dc3545', 'BUG': '#dc3545',
    'HACK': '#e67700', 'XXX': '#e67700', 'NOTE': '#0dcaf0'
  }};
  drawBarChart(
    'chart-tags',
    D.tag_counts.map(t => t[0]),
    D.tag_counts.map(t => t[1]),
    D.tag_counts.map(t => tagColors[t[0]] || '#6c757d')
  );

  // Priority chart
  drawBarChart(
    'chart-priority',
    ['Normal', 'High', 'Urgent'],
    [D.priority_counts.normal, D.priority_counts.high, D.priority_counts.urgent],
    ['#51cf66', '#ff922b', '#ff6b6b']
  );

  // Authors list
  function renderBarList(containerId, items) {{
    const el = document.getElementById(containerId);
    const max = items.length > 0 ? items[0][1] : 1;
    items.forEach(([name, count]) => {{
      const row = document.createElement('div');
      row.className = 'bar-container';
      row.style.marginBottom = '4px';
      const pct = Math.max((count / max) * 100, 2);
      row.innerHTML = '<span style="width:140px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">' + escapeHtml(name) + '</span>' +
        '<div class="bar" style="width:' + pct + '%;"></div>' +
        '<span style="font-size:0.85rem;">' + count + '</span>';
      el.appendChild(row);
    }});
    if (items.length === 0) {{
      el.innerHTML = '<p style="color:var(--text-muted)">No data</p>';
    }}
  }}
  renderBarList('authors-list', D.author_counts);
  renderBarList('hotspots-list', D.hotspot_files);

  // Items table
  const tbody = document.querySelector('#items-table tbody');
  D.items.forEach(item => {{
    const tr = document.createElement('tr');
    const priorityClass = item.priority === 'urgent' ? 'priority-urgent' : item.priority === 'high' ? 'priority-high' : '';
    tr.innerHTML =
      '<td>' + escapeHtml(item.file) + '</td>' +
      '<td>' + item.line + '</td>' +
      '<td><span class="tag tag-' + item.tag + '">' + item.tag + '</span></td>' +
      '<td class="' + priorityClass + '">' + item.priority + '</td>' +
      '<td>' + escapeHtml(item.message) + '</td>' +
      '<td>' + escapeHtml(item.author || '') + '</td>';
    tbody.appendChild(tr);
  }});

  // Sortable table
  let sortCol = 'file', sortAsc = true;
  document.querySelectorAll('#items-table th').forEach(th => {{
    th.addEventListener('click', () => {{
      const col = th.dataset.col;
      if (sortCol === col) sortAsc = !sortAsc; else {{ sortCol = col; sortAsc = true; }}
      const rows = Array.from(tbody.querySelectorAll('tr'));
      const colIdx = Array.from(th.parentNode.children).indexOf(th);
      rows.sort((a, b) => {{
        let va = a.children[colIdx].textContent;
        let vb = b.children[colIdx].textContent;
        if (col === 'line') {{ va = parseInt(va, 10); vb = parseInt(vb, 10); return sortAsc ? va - vb : vb - va; }}
        return sortAsc ? va.localeCompare(vb) : vb.localeCompare(va);
      }});
      rows.forEach(r => tbody.appendChild(r));
    }});
  }});

  function escapeHtml(s) {{
    const div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
  }}
}})();
</script>
</body>
</html>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn minimal_report() -> ReportResult {
        ReportResult {
            generated_at: "2024-01-01T00:00:00Z".to_string(),
            summary: ReportSummary {
                total_items: 0,
                total_files: 0,
                files_scanned: 0,
                urgent_count: 0,
                high_count: 0,
                stale_count: 0,
                avg_age_days: 0,
            },
            tag_counts: vec![],
            priority_counts: PriorityCounts {
                normal: 0,
                high: 0,
                urgent: 0,
            },
            author_counts: vec![],
            hotspot_files: vec![],
            history: vec![],
            age_histogram: vec![],
            items: vec![],
        }
    }

    #[test]
    fn test_render_html_contains_doctype() {
        let html = render_html(&minimal_report());
        assert!(html.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn test_render_html_embeds_valid_json() {
        let mut report = minimal_report();
        report.items.push(TodoItem {
            file: "test.rs".to_string(),
            line: 1,
            tag: Tag::Todo,
            message: "hello world".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        });
        let html = render_html(&report);
        // Extract JSON from REPORT_DATA
        let start = html.find("const REPORT_DATA = ").unwrap() + "const REPORT_DATA = ".len();
        let end = html[start..].find(";\n").unwrap() + start;
        let json_str = &html[start..end];
        let parsed: serde_json::Value = serde_json::from_str(json_str).expect("JSON should parse");
        assert_eq!(parsed["items"][0]["message"], "hello world");
    }

    #[test]
    fn test_render_html_escapes_script_tags() {
        let mut report = minimal_report();
        report.items.push(TodoItem {
            file: "test.rs".to_string(),
            line: 1,
            tag: Tag::Todo,
            message: "has </script> in it".to_string(),
            author: None,
            issue_ref: None,
            priority: Priority::Normal,
            deadline: None,
        });
        let html = render_html(&report);
        // The raw </script> should not appear inside our <script> block
        // (it should be escaped to <\/script>)
        let script_start = html.find("const REPORT_DATA = ").unwrap();
        let script_end = html[script_start..].find("</script>").unwrap() + script_start;
        let script_content = &html[script_start..script_end];
        assert!(
            !script_content.contains("</script>"),
            "JSON data should not contain raw </script>"
        );
    }

    #[test]
    fn test_render_html_bar_list_escapes_html_in_names() {
        let mut report = minimal_report();
        let xss_author = "<img src=x onerror=alert(1)>";
        report.author_counts.push((xss_author.to_string(), 5));
        let html = render_html(&report);
        // The JavaScript renderBarList() must use escapeHtml() on `name`,
        // so the raw HTML tag should not appear unescaped in the template.
        // We verify the JS source calls escapeHtml(name) rather than bare name.
        assert!(
            html.contains("escapeHtml(name)"),
            "renderBarList must escape author names with escapeHtml()"
        );
    }

    #[test]
    fn test_render_html_escapes_script_tag_case_insensitive() {
        for variant in ["</Script>", "</SCRIPT>", "</sCrIpT>"] {
            let mut report = minimal_report();
            report.items.push(TodoItem {
                file: "test.rs".to_string(),
                line: 1,
                tag: Tag::Todo,
                message: format!("xss attempt {variant}"),
                author: None,
                issue_ref: None,
                priority: Priority::Normal,
                deadline: None,
            });
            let html = render_html(&report);
            let script_start = html.find("const REPORT_DATA = ").unwrap();
            let script_end = html[script_start..].find("</script>").unwrap() + script_start;
            let script_content = &html[script_start..script_end];
            // No case variant of </script> should appear in JSON data
            assert!(
                !script_content.to_lowercase().contains("</script>"),
                "JSON data must not contain {variant} — would break the script block"
            );
        }
    }
}
