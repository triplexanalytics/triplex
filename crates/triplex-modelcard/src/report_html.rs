//! `report.html`: the standalone offline model card. Inline CSS only, no external resource, so
//! it opens correctly from a local file with no network access.

use crate::{CheckOutcome, GroupResult, Metrics, ModelCardReport, Status};

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "undefined".to_string(), |x| format!("{x:.4}"))
}

fn metrics_cells(m: &Metrics) -> String {
    format!(
        "<td>{:.4}</td><td>{:.4}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>",
        m.selection_rate,
        m.accuracy,
        num(m.precision),
        num(m.recall),
        num(m.fpr),
        num(m.f1),
        num(m.roc_auc),
    )
}

fn group_row(g: &GroupResult, min: usize) -> String {
    match &g.metrics {
        Some(m) => format!("<tr><td>{}</td><td>{}</td>{}</tr>", escape(&g.group), g.n, metrics_cells(m)),
        None => format!(
            "<tr class=\"suppressed\"><td>{}</td><td>{}</td><td colspan=\"7\">suppressed (n below {min})</td></tr>",
            escape(&g.group),
            g.n
        ),
    }
}

const TABLE_HEAD: &str = "<thead><tr><th>Group</th><th>n</th><th>Selection rate</th><th>Accuracy</th><th>Precision</th><th>Recall (TPR)</th><th>FPR</th><th>F1</th><th>ROC-AUC</th></tr></thead>";

pub fn render_html(report: &ModelCardReport) -> String {
    let (label, class) = match report.status {
        Status::Pass => ("PASS", "status-pass"),
        Status::Warn => ("WARN", "status-warn"),
        Status::Fail => ("FAIL", "status-fail"),
    };
    let min = report.data.min_slice_size;
    let model = |v: &Option<String>| {
        v.as_deref()
            .map_or_else(|| "not declared".to_string(), escape)
    };

    let slices: String = report
        .slices
        .iter()
        .map(|s| {
            let rows: String = s.groups.iter().map(|g| group_row(g, min)).collect();
            format!(
                "<h3>Slice: {}</h3><table>{TABLE_HEAD}<tbody>{rows}</tbody></table>",
                escape(&s.column)
            )
        })
        .collect();

    let fairness = match &report.fairness {
        None => "<p>No protected attribute was configured, so no fairness checks were run.</p>"
            .to_string(),
        Some(f) => {
            let checks: String = f
                .checks
                .iter()
                .map(|c| {
                    let (word, cls) = match c.outcome {
                        CheckOutcome::Pass => ("pass", "status-pass"),
                        CheckOutcome::Fail => ("fail", "status-fail"),
                        CheckOutcome::Undefined => ("undefined", "status-warn"),
                        CheckOutcome::Informational => ("informational", ""),
                    };
                    let kind = match c.kind {
                        crate::CheckKind::Min => "at least",
                        crate::CheckKind::Max => "at most",
                    };
                    let threshold = c.threshold.map_or_else(|| "no threshold".to_string(), |t| format!("{kind} {t:.4}"));
                    format!("<tr><td>{}</td><td>{}</td><td>{}</td><td><span class=\"status {cls}\">{word}</span></td></tr>", escape(&c.name), num(c.value), threshold)
                })
                .collect();
            let compared = f
                .groups_compared
                .iter()
                .map(|g| escape(g))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "<p>Protected attribute: <strong>{}</strong>. Groups compared: {}. Suppressed groups (too small): {}.</p>\
                 <table><thead><tr><th>Check</th><th>Value</th><th>Threshold</th><th>Outcome</th></tr></thead><tbody>{checks}</tbody></table>\
                 <p class=\"note\">Ratios are min/max across groups; differences are max-min. Equalized odds combines the true-positive and false-positive rate gaps (larger difference, smaller ratio).</p>",
                escape(&f.protected_attribute),
                if compared.is_empty() { "none".to_string() } else { compared },
                if f.groups_suppressed.is_empty() { "none".to_string() } else { f.groups_suppressed.iter().map(|g| escape(g)).collect::<Vec<_>>().join(", ") },
            )
        }
    };

    let warnings = if report.warnings.is_empty() {
        String::new()
    } else {
        format!(
            "<h2>Warnings</h2><ul>{}</ul>",
            report
                .warnings
                .iter()
                .map(|w| format!("<li>{}</li>", escape(w)))
                .collect::<String>()
        )
    };

    let aid: String = report
        .documentation_aid
        .sections
        .iter()
        .map(|s| {
            format!(
                "<li><strong>{}</strong>: {} <em>(lines up with: {})</em></li>",
                escape(s.heading),
                escape(s.covers),
                escape(&s.aligned_with.join("; "))
            )
        })
        .collect();

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>ModelCard report</title>
<style>
  body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #1a1a1a; background: #fff; max-width: 70rem; }}
  h1 {{ font-size: 1.3rem; }} h2 {{ font-size: 1.1rem; margin-top: 2rem; }} h3 {{ font-size: 1rem; }}
  .status {{ display: inline-block; padding: 0.15rem 0.6rem; border-radius: 0.25rem; font-weight: 600; }}
  .status-pass {{ background: #e6f4ea; color: #1e7e34; }}
  .status-warn {{ background: #fff4e5; color: #a15c00; }}
  .status-fail {{ background: #fdeaea; color: #b00020; }}
  .disclaimer {{ border-left: 4px solid #a15c00; background: #fff8ec; padding: 0.6rem 1rem; margin: 1rem 0; }}
  table {{ border-collapse: collapse; width: 100%; margin: 0.5rem 0 1rem; }}
  th, td {{ text-align: left; padding: 0.4rem 0.5rem; border-bottom: 1px solid #ddd; font-size: 0.88rem; }}
  tr.suppressed td {{ color: #777; font-style: italic; }}
  .note {{ color: #555; font-size: 0.85rem; }}
</style>
</head>
<body>
<h1>ModelCard report <span class="status {class}">{label}</span></h1>
<div class="disclaimer">{disclaimer}</div>
<h2>Model</h2>
<p>Name: {name}; version: {version}; framework: {framework}; intended use: {intended}; training data: {training}</p>
<h2>Data</h2>
<p>{used} of {read} rows used ({dropped} dropped for an empty label or prediction). Positive label: <code>{positive}</code>. Minimum slice size: {min}.</p>
<h2>Overall</h2>
<table>{TABLE_HEAD}<tbody><tr><td>all rows</td><td>{used}</td>{overall}</tr></tbody></table>
<h2>Slices</h2>
{slices}
<h2>Fairness</h2>
{fairness}
{warnings}
<h2>Documentation structure</h2>
<ul>{aid}</ul>
</body>
</html>
"#,
        disclaimer = escape(report.documentation_aid.disclaimer),
        name = model(&report.model.name),
        version = model(&report.model.version),
        framework = model(&report.model.framework),
        intended = model(&report.model.intended_use),
        training = model(&report.model.training_data),
        used = report.data.rows_used,
        read = report.data.rows_read,
        dropped = report.data.rows_dropped_missing_label,
        positive = escape(&report.data.positive_label),
        overall = metrics_cells(&report.overall),
    )
}
