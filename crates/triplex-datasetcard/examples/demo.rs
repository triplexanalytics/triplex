//! Prints every risk model this crate can compute against one small
//! synthetic dataset, for evaluating the new metrics from the design notes
//!  against real numbers before deciding on any UI investment. Run
//! with `cargo run --example demo -p triplex-datasetcard`.

use triplex_datasetcard::{
    beta_likeness, delta_disclosure, dis_suda_scores, k_anonymity, l_diversity, suda_scores,
    t_closeness,
};

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| v.to_string()).collect()
}

fn main() {
    // Same shape as this session's browser demo file: id (unique), age
    // (bucketed to a decade), city/plan (categorical strings, exact-value
    // grouped — no generalization hierarchy applied here).
    let age_bucket = strings(&[
        "30-39", "20-29", "40-49", "30-39", "50-59", "20-29", "40-49", "60-69", "30-39", "20-29",
    ]);
    let city = strings(&[
        "Utrecht", "Utrecht", "Rotterdam", "Utrecht", "Amsterdam", "Rotterdam", "Utrecht", "Zurich",
        "Utrecht", "Rotterdam",
    ]);
    let plan = strings(&[
        "free", "pro", "free", "pro", "enterprise", "free", "pro", "free", "free", "pro",
    ]);

    println!("=== Column-level (age bucket vs. plan as the sensitive attribute) ===");
    println!("k-anonymity:      {}", k_anonymity(&age_bucket));
    println!("l-diversity:      {}", l_diversity(&age_bucket, &plan));
    println!("t-closeness:      {:.4}", t_closeness(&age_bucket, &plan));
    println!("beta-likeness:    {:.4}", beta_likeness(&age_bucket, &plan));
    println!("delta-disclosure: {:.4}", delta_disclosure(&age_bucket, &plan));

    println!("\n=== Column-level (city, exact-value, vs. plan) ===");
    println!("k-anonymity:      {}", k_anonymity(&city));
    println!("l-diversity:      {}", l_diversity(&city, &plan));
    println!("t-closeness:      {:.4}", t_closeness(&city, &plan));
    println!("beta-likeness:    {:.4}", beta_likeness(&city, &plan));
    println!("delta-disclosure: {:.4}", delta_disclosure(&city, &plan));

    println!("\n=== SUDA / DIS-SUDA (row-level, columns: age bucket + city + plan) ===");
    let suda = suda_scores(&[age_bucket.clone(), city.clone(), plan.clone()]).unwrap();
    let dis_suda = dis_suda_scores(&suda);
    let names = [
        "Utrecht/30-39/free",
        "Utrecht/20-29/pro",
        "Rotterdam/40-49/free",
        "Utrecht/30-39/pro",
        "Amsterdam/50-59/enterprise",
        "Rotterdam/20-29/free",
        "Utrecht/40-49/pro",
        "Zurich/60-69/free",
        "Utrecht/30-39/free",
        "Rotterdam/20-29/pro",
    ];
    for (i, name) in names.iter().enumerate() {
        println!(
            "row {i} ({name:28}): suda={:>5.1}  dis-suda share={:.1}%",
            suda[i],
            dis_suda[i] * 100.0
        );
    }
}
