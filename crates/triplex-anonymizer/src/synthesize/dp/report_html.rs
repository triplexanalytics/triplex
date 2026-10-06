//! `report.html` for a DP synthesis run: inline CSS only, no external resource. The privacy
//! section comes first; real-data diagnostics appear only when they were explicitly attached, in a
//! block that says they are outside the guarantee.

use super::DpReport;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("{x:.4}"))
}

pub fn render_html(r: &DpReport) -> String {
    let p = &r.privacy;
    let ledger: String = p
        .ledger
        .iter()
        .map(|s| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{:.6}</td><td>{:.6}</td></tr>",
                esc(&s.mechanism),
                s.sensitivity,
                s.epsilon_allotted,
                s.epsilon_used
            )
        })
        .collect();
    let warnings: String = r
        .warnings
        .iter()
        .map(|w| format!("<li>{}</li>", esc(w)))
        .collect();
    let limits: String = r
        .limitations
        .iter()
        .map(|l| format!("<li>{}</li>", esc(l)))
        .collect();
    let diagnostics = match &r.nonprivate_diagnostics {
        None => String::new(),
        Some(d) => {
            let rows: String = d
                .fidelity
                .columns
                .iter()
                .map(|c| {
                    format!(
                        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        esc(&c.column),
                        c.kind,
                        num(c.ks_statistic),
                        num(c.total_variation)
                    )
                })
                .collect();
            format!(
                "<h2>Non-private diagnostics</h2><div class=\"warn\">{}</div>\
                 <table><thead><tr><th>Column</th><th>Kind</th><th>KS</th><th>Total variation</th></tr></thead><tbody>{rows}</tbody></table>\
                 <p>Exact copies of a real row: {} ({:.2}%). Numeric correlation max error {}.</p>",
                esc(d.warning),
                d.leakage.exact_copy_rows,
                d.leakage.exact_copy_rate * 100.0,
                num(d.fidelity.correlation.max_abs_error),
            )
        }
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>DP synthesis report</title>
<style>
  body {{ font-family: system-ui, sans-serif; margin: 2rem; color: #1a1a1a; background: #fff; max-width: 70rem; }}
  h1 {{ font-size: 1.3rem; }} h2 {{ font-size: 1.1rem; margin-top: 2rem; }}
  .guarantee {{ border-left: 4px solid #1e7e34; background: #e6f4ea; padding: 0.6rem 1rem; margin: 1rem 0; }}
  .warn {{ border-left: 4px solid #b00020; background: #fdeaea; padding: 0.6rem 1rem; margin: 1rem 0; }}
  table {{ border-collapse: collapse; width: 100%; margin: 0.5rem 0 1rem; }}
  th, td {{ text-align: left; padding: 0.4rem 0.5rem; border-bottom: 1px solid #ddd; font-size: 0.88rem; }}
</style>
</head>
<body>
<h1>DP synthesis report ({method})</h1>
<div class="guarantee"><strong>{guarantee}: epsilon = {eps}</strong> (delta = {delta}). Neighbor relation: {neighbor}. Row count public: {rows_public}. Column list and bounds public: {bounds_public}.<br>{statement}</div>
<p>{rows_in} rows in, {rows_out} rows out; columns: {columns}. Budget spent: {spent:.6} of {eps}. Sampling seed {seed} (sampling only; the noise never uses it).</p>
{network}
<h2>Mechanisms</h2>
<ul><li>Marginals: {marginal}</li><li>Correlation: {correlation}</li><li>Noise source: {source}</li><li>{rounding}</li></ul>
<h2>Budget ledger</h2>
<table><thead><tr><th>Mechanism</th><th>Sensitivity</th><th>Epsilon allotted</th><th>Epsilon used</th></tr></thead><tbody>{ledger}</tbody></table>
<h2>Warnings</h2><ul>{warnings}</ul>
<h2>Limitations</h2><ul>{limits}</ul>
{diagnostics}
</body>
</html>
"#,
        network = super::super::bayesnet::network_html(r.network.as_ref()),
        method = r.method,
        guarantee = esc(p.guarantee),
        eps = p.epsilon_total,
        delta = p.delta,
        neighbor = esc(p.neighbor_relation),
        rows_public = p.row_count_public,
        bounds_public = p.bounds_and_columns_public,
        statement = esc(p.statement),
        rows_in = r.rows_in,
        rows_out = r.rows_out,
        columns = esc(&r.columns.join(", ")),
        spent = p.epsilon_spent,
        seed = r.sampling_seed,
        marginal = esc(p.marginal_mechanism),
        correlation = esc(p.correlation_mechanism),
        source = esc(p.noise_source),
        rounding = esc(p.epsilon_rounding),
    )
}
