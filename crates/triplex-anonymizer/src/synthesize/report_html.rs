//! `report.html` for a synthesis run: inline CSS only, no external resource.

use super::SynthesisReport;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("{x:.4}"))
}

pub fn render_html(r: &SynthesisReport) -> String {
    let columns: String = r
        .fidelity
        .columns
        .iter()
        .map(|c| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} / {}</td><td>{}</td></tr>",
                esc(&c.column),
                c.kind,
                num(c.ks_statistic),
                num(c.total_variation),
                num(c.mean_real),
                num(c.mean_synthetic),
                format_args!("{:.3} / {:.3}", c.null_rate_real, c.null_rate_synthetic),
            )
        })
        .collect();
    let dropped: String = if r.columns_dropped.is_empty() {
        "<li>none</li>".into()
    } else {
        r.columns_dropped
            .iter()
            .map(|d| {
                format!(
                    "<li><strong>{}</strong>: {}</li>",
                    esc(&d.column),
                    esc(&d.reason)
                )
            })
            .collect()
    };
    let limits: String = r
        .limitations
        .iter()
        .map(|l| format!("<li>{}</li>", esc(l)))
        .collect();
    let l = &r.leakage;
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Synthesis report</title>
<style>
  body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #1a1a1a; background: #fff; max-width: 70rem; }}
  h1 {{ font-size: 1.3rem; }} h2 {{ font-size: 1.1rem; margin-top: 2rem; }}
  .notice {{ border-left: 4px solid #a15c00; background: #fff8ec; padding: 0.6rem 1rem; margin: 1rem 0; }}
  table {{ border-collapse: collapse; width: 100%; margin: 0.5rem 0 1rem; }}
  th, td {{ text-align: left; padding: 0.4rem 0.5rem; border-bottom: 1px solid #ddd; font-size: 0.88rem; }}
</style>
</head>
<body>
<h1>Synthesis report ({method}, seed {seed})</h1>
<div class="notice">{privacy}</div>
<p>{rows_in} real rows in, {rows_out} synthetic rows out. {n_cols} columns synthesized.</p>
{network}
<h2>Fidelity per column</h2>
<table><thead><tr><th>Column</th><th>Kind</th><th>KS statistic</th><th>Total variation</th><th>Mean real / synthetic</th><th>Null rate real / synthetic</th></tr></thead><tbody>{columns}</tbody></table>
<p>Numeric correlation: {pairs} pair(s) compared, max absolute error {max_err}, mean {mean_err}. <em>{corr_note}</em></p>
<h2>Leakage (measured, not certified)</h2>
<p>Exact copies of a real row: {copies} of {rows_out} ({rate:.2}%). Nearest real record (synthetic): min {dmin}, 5th percentile {p5}, median {dmed}. Real-to-real median: {rmed}. Ratio: {ratio}.</p>
<p><em>{leak_note}</em></p>
<h2>Not synthesized</h2>
<ul>{dropped}</ul>
<h2>Limitations</h2>
<ul>{limits}</ul>
</body>
</html>
"#,
        network = super::bayesnet::network_html(r.network.as_ref()),
        method = r.method,
        seed = r.seed,
        privacy = esc(r.privacy_statement),
        rows_in = r.rows_in,
        rows_out = r.rows_out,
        n_cols = r.columns_synthesized.len(),
        pairs = r.fidelity.correlation.pairs_compared,
        max_err = num(r.fidelity.correlation.max_abs_error),
        mean_err = num(r.fidelity.correlation.mean_abs_error),
        corr_note = esc(r.fidelity.correlation.note),
        copies = l.exact_copy_rows,
        rate = l.exact_copy_rate * 100.0,
        dmin = num(l.dcr_synthetic_min),
        p5 = num(l.dcr_synthetic_p5),
        dmed = num(l.dcr_synthetic_median),
        rmed = num(l.dcr_real_to_real_median),
        ratio = num(l.dcr_ratio),
        leak_note = esc(l.note),
    )
}
