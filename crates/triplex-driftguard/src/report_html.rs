//! `report.html`: the standalone offline visual report from
//! the design notes's CI/CD output contract --
//! inline CSS only, no external stylesheet/font/script, so it opens
//! correctly from a local file with zero network access.

use crate::{DriftFinding, DriftReport};

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn severity_class(finding: &DriftFinding) -> &'static str {
    if finding.is_regression() {
        "regression"
    } else {
        "drift"
    }
}

fn finding_row(finding: &DriftFinding) -> String {
    let (name, description) = match finding {
        DriftFinding::ColumnAdded { name } => {
            (name.clone(), "new column, not in the baseline".to_string())
        }
        DriftFinding::ColumnRemoved { name } => (
            name.clone(),
            "column missing from the candidate".to_string(),
        ),
        DriftFinding::TypeChanged {
            name,
            baseline_type,
            candidate_type,
        } => (
            name.clone(),
            format!("type changed: {baseline_type:?} -> {candidate_type:?}"),
        ),
        DriftFinding::NullRateDrift {
            name,
            baseline_percentage,
            candidate_percentage,
            delta,
            threshold,
        } => (
            name.clone(),
            format!(
                "null rate {baseline_percentage:.2}% -> {candidate_percentage:.2}% \
                 (delta {delta:.2}pp, threshold {threshold:.2}pp)"
            ),
        ),
        DriftFinding::MeanDrift {
            name,
            baseline_mean,
            candidate_mean,
            relative_delta,
            threshold,
        } => (
            name.clone(),
            format!(
                "mean {baseline_mean:.4} -> {candidate_mean:.4} \
                 (relative delta {relative_delta:.2}, threshold {threshold:.2})"
            ),
        ),
        DriftFinding::TypeConsistencyDrift {
            name,
            baseline_percentage,
            candidate_percentage,
            delta,
            threshold,
        } => (
            name.clone(),
            format!(
                "type consistency {baseline_percentage:.2}% -> {candidate_percentage:.2}% \
                 (delta {delta:.2}pp, threshold {threshold:.2}pp)"
            ),
        ),
        DriftFinding::RiskDegraded {
            model,
            baseline,
            candidate,
        } => (
            format!("({model})"),
            format!("risk assessment worsened: {baseline:.4} -> {candidate:.4}"),
        ),
    };
    format!(
        "<tr class=\"{class}\"><td>{name}</td><td>{severity}</td><td>{description}</td></tr>",
        class = severity_class(finding),
        name = escape(&name),
        severity = if finding.is_regression() {
            "regression"
        } else {
            "drift"
        },
        description = escape(&description),
    )
}

/// Renders a self-contained HTML report for a `DriftReport` -- no external
/// resources, so it stays readable offline and in an air-gapped CI runner.
pub fn render_html(report: &DriftReport, baseline_label: &str, candidate_label: &str) -> String {
    let status = if report.has_regression {
        ("FAIL", "status-fail")
    } else if report.has_drift {
        ("WARN", "status-warn")
    } else {
        ("PASS", "status-pass")
    };

    let rows: String = if report.findings.is_empty() {
        "<tr><td colspan=\"3\">No drift detected.</td></tr>".to_string()
    } else {
        report.findings.iter().map(finding_row).collect()
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>DriftGuard report</title>
<style>
  body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #1a1a1a; background: #fff; }}
  h1 {{ font-size: 1.25rem; }}
  .meta {{ color: #555; margin-bottom: 1rem; }}
  .status {{ display: inline-block; padding: 0.25rem 0.75rem; border-radius: 0.25rem; font-weight: 600; }}
  .status-pass {{ background: #e6f4ea; color: #1e7e34; }}
  .status-warn {{ background: #fff4e5; color: #a15c00; }}
  .status-fail {{ background: #fdeaea; color: #b00020; }}
  table {{ border-collapse: collapse; width: 100%; margin-top: 1rem; }}
  th, td {{ text-align: left; padding: 0.5rem; border-bottom: 1px solid #ddd; font-size: 0.9rem; }}
  tr.regression td {{ background: #fdeaea; }}
  tr.drift td {{ background: #fff8ec; }}
</style>
</head>
<body>
<h1>DriftGuard report <span class="status {status_class}">{status_label}</span></h1>
<p class="meta">baseline: {baseline} &middot; candidate: {candidate}</p>
<table>
<thead><tr><th>Column</th><th>Severity</th><th>Finding</th></tr></thead>
<tbody>
{rows}
</tbody>
</table>
</body>
</html>
"#,
        status_class = status.1,
        status_label = status.0,
        baseline = escape(baseline_label),
        candidate = escape(candidate_label),
        rows = rows,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pass_status_with_no_findings() {
        let report = DriftReport {
            has_regression: false,
            has_drift: false,
            findings: vec![],
        };
        let html = render_html(&report, "base.csv", "candidate.csv");
        assert!(html.contains("PASS"));
        assert!(html.contains("No drift detected."));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn renders_fail_status_and_escapes_finding_names() {
        let report = DriftReport {
            has_regression: true,
            has_drift: true,
            findings: vec![DriftFinding::ColumnRemoved {
                name: "<script>".to_string(),
            }],
        };
        let html = render_html(&report, "base.csv", "candidate.csv");
        assert!(html.contains("FAIL"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
