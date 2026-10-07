//! Tier 3, slice T3.1: sketch-based synthesis. Synthetic fixtures only.

use triplex_anonymizer::synthesize::sketch::{
    generate, propose, sketch_from_yaml, sketch_to_yaml, synthesize_sketch, Axis, ColumnKind,
    Sketch, SketchClass, SketchColumn, SketchConfig, DEFAULT_MIN_CLASS_SIZE, SKETCH_VERSION,
};
use triplex_anonymizer::synthesize::SynthesizeConfig;
use triplex_io::Sample;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn unit(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

const PLANS: [&str; 3] = ["free", "pro", "enterprise"];
const REGIONS: [&str; 4] = ["north", "south", "east", "west"];

/// `region` follows `plan` 90% of the time and `spend` sits in a band per plan; `score` is noise.
fn customers(n: usize) -> Sample {
    let mut rng = Lcg(7);
    let rows = (0..n)
        .map(|_| {
            let p = match rng.unit() {
                x if x < 0.5 => 0,
                x if x < 0.8 => 1,
                _ => 2,
            };
            let region = if rng.unit() < 0.9 {
                p
            } else {
                rng.below(4) as usize
            };
            let spend = [10.0, 200.0, 900.0][p] + rng.unit() * 50.0;
            vec![
                PLANS[p].to_string(),
                REGIONS[region].to_string(),
                format!("{spend:.2}"),
                format!("{:.1}", rng.unit() * 100.0),
            ]
        })
        .collect();
    Sample {
        headers: vec![
            "plan".into(),
            "region".into(),
            "spend".into(),
            "score".into(),
        ],
        rows,
    }
}

fn agreement(sample: &Sample) -> f64 {
    let hits = sample
        .rows
        .iter()
        .filter(|r| {
            let p = PLANS.iter().position(|x| *x == r[0]);
            let g = REGIONS.iter().position(|x| *x == r[1]);
            matches!((p, g), (Some(p), Some(g)) if p == g)
        })
        .count();
    hits as f64 / sample.rows.len() as f64
}

fn config(classes: usize) -> SketchConfig {
    SketchConfig {
        classes,
        ..SketchConfig::default()
    }
}

/// Two classes on one number column (`x`) and one category column (`kind`), far apart.
fn two_class_sketch() -> Sketch {
    let class = |name: &str, size, min, max, shares: [f64; 2]| SketchClass {
        name: name.into(),
        size,
        axes: vec![
            Axis::Numeric {
                min,
                max,
                density: vec![1.0, 1.0],
                null_share: 0.0,
            },
            Axis::Categorical {
                shares: shares.to_vec(),
                null_share: 0.0,
            },
        ],
    };
    Sketch {
        version: SKETCH_VERSION,
        columns: vec![
            SketchColumn {
                name: "x".into(),
                kind: ColumnKind::Numeric { integer: false },
            },
            SketchColumn {
                name: "kind".into(),
                kind: ColumnKind::Categorical {
                    values: vec!["a".into(), "b".into()],
                },
            },
        ],
        classes: vec![
            class("low", 300, 0.0, 10.0, [1.0, 0.0]),
            class("high", 100, 100.0, 110.0, [0.0, 1.0]),
        ],
    }
}

fn numbers(columns: &[Vec<Option<String>>], column: usize) -> Vec<f64> {
    columns[column]
        .iter()
        .map(|v| v.as_ref().unwrap().parse().unwrap())
        .collect()
}

fn share_below(values: &[f64], limit: f64) -> f64 {
    values.iter().filter(|v| **v < limit).count() as f64 / values.len() as f64
}

// ---- the sketch as a file

#[test]
fn a_sketch_survives_the_config_block_and_json_unchanged() {
    let sketch = propose(&customers(600), &config(3)).unwrap();
    let yaml = sketch_to_yaml(&sketch).unwrap();
    assert!(yaml.contains("sketch:"));
    assert_eq!(sketch_from_yaml(&yaml).unwrap(), sketch);
    let json = serde_json::to_string(&sketch).unwrap();
    assert_eq!(serde_json::from_str::<Sketch>(&json).unwrap(), sketch);
    assert!(sketch_from_yaml("version: 1\ncolumns: []\n")
        .unwrap_err()
        .to_string()
        .contains("no `sketch:` block"));
}

#[test]
fn an_unusable_sketch_is_refused_with_the_class_and_column_named() {
    let broken = |edit: &dyn Fn(&mut Sketch)| {
        let mut sketch = two_class_sketch();
        edit(&mut sketch);
        sketch.validate().unwrap_err().to_string()
    };
    assert!(broken(&|s| s.classes.clear()).contains("no classes"));
    assert!(broken(&|s| s.classes.iter_mut().for_each(|c| c.size = 0)).contains("size 0"));
    assert!(broken(&|s| s.version = 99).contains("not supported"));
    assert!(broken(&|s| {
        s.classes[0].axes.pop();
    })
    .contains("1 axis entries for 2 columns"));
    let range = broken(&|s| {
        s.classes[1].axes[0] = Axis::Numeric {
            min: 5.0,
            max: 1.0,
            density: vec![1.0],
            null_share: 0.0,
        }
    });
    assert!(
        range.contains("class \"high\", column \"x\"") && range.contains("minimum"),
        "{range}"
    );
    assert!(broken(&|s| {
        s.classes[0].axes[1] = Axis::Categorical {
            shares: vec![1.0],
            null_share: 0.0,
        }
    })
    .contains("1 shares for 2 values"));
    assert!(broken(&|s| {
        s.classes[0].axes[1] = Axis::Categorical {
            shares: vec![0.5, -0.5],
            null_share: 0.0,
        }
    })
    .contains("no negative one"));
    assert!(broken(&|s| s.classes[0].axes.swap(0, 1)).contains("does not match"));
    // a negative size cannot even be read
    let yaml = sketch_to_yaml(&two_class_sketch())
        .unwrap()
        .replace("size: 300", "size: -3");
    assert!(sketch_from_yaml(&yaml).is_err());
}

// ---- proposing from the data

#[test]
fn a_proposal_is_deterministic_and_respects_the_minimum_class_size() {
    let real = customers(1200);
    let first = propose(&real, &config(4)).unwrap();
    assert_eq!(first, propose(&real, &config(4)).unwrap());
    first.validate().unwrap();
    assert!(first.classes.len() <= 4 && first.classes.len() >= 2);
    assert_eq!(first.classes.iter().map(|c| c.size).sum::<usize>(), 1200);
    assert!(first
        .classes
        .iter()
        .all(|c| c.size >= DEFAULT_MIN_CLASS_SIZE));
    // largest first
    assert!(first.classes.windows(2).all(|w| w[0].size >= w[1].size));

    let mut other_seed = config(4);
    other_seed.base.seed = 9;
    propose(&real, &other_seed).unwrap().validate().unwrap();
}

#[test]
fn the_minimum_class_size_is_the_users_to_set() {
    let real = customers(1200);
    // asking for many classes with a high floor leaves only classes above the floor
    let mut strict = config(12);
    strict.min_class_size = 250;
    let sketch = propose(&real, &strict).unwrap();
    assert!(
        sketch.classes.iter().all(|c| c.size >= 250),
        "{:?}",
        sketch.classes.iter().map(|c| c.size).collect::<Vec<_>>()
    );
    assert_eq!(sketch.classes.iter().map(|c| c.size).sum::<usize>(), 1200);
    // a floor above the row count leaves one class holding everything
    strict.min_class_size = 5000;
    let one = propose(&real, &strict).unwrap();
    assert_eq!(one.classes.len(), 1);
    assert_eq!(one.classes[0].size, 1200);
    // a floor of 1 folds nothing
    let mut loose = config(12);
    loose.min_class_size = 1;
    assert!(propose(&real, &loose).unwrap().classes.len() > sketch.classes.len());

    for (edit, message) in [
        (
            (&|c: &mut SketchConfig| c.min_class_size = 0) as &dyn Fn(&mut SketchConfig),
            "minimum class size",
        ),
        (&|c: &mut SketchConfig| c.classes = 0, "at least 1 class"),
        (&|c: &mut SketchConfig| c.bins = 0, "density bin"),
    ] {
        let mut bad = config(4);
        edit(&mut bad);
        let error = propose(&real, &bad).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
}

// ---- generating

#[test]
fn separated_classes_keep_a_dependency_that_one_class_loses() {
    let real = customers(3000);
    let base = SynthesizeConfig::default();
    assert!(agreement(&real) > 0.9);

    let several = propose(&real, &config(4)).unwrap();
    let kept = synthesize_sketch(&real, &several, &base).unwrap();
    assert!(
        agreement(&kept.sample) > 0.8,
        "classes should keep plan and region together: {}",
        agreement(&kept.sample)
    );

    // one class is independent marginals: about 0.37 agreement, the product of the shares
    let one = propose(&real, &config(1)).unwrap();
    assert_eq!(one.classes.len(), 1);
    let lost = synthesize_sketch(&real, &one, &base).unwrap();
    assert!(
        agreement(&lost.sample) < 0.45,
        "{}",
        agreement(&lost.sample)
    );
    // ... while each column on its own still matches
    for column in &lost.report.fidelity.columns {
        if let Some(tv) = column.total_variation {
            assert!(tv < 0.05, "{} {tv}", column.column);
        }
    }
}

#[test]
fn every_value_lies_in_its_class_and_classes_appear_in_proportion_to_their_size() {
    let sketch = two_class_sketch();
    let columns = generate(&sketch, 8000, 1);
    let x = numbers(&columns, 0);
    assert!(x
        .iter()
        .all(|v| (0.0..=10.0).contains(v) || (100.0..=110.0).contains(v)));
    // class "low" holds 300 of 400
    assert!((share_below(&x, 50.0) - 0.75).abs() < 0.02);
    // the category goes with the class: no "b" in the low range, no "a" in the high one
    for (value, kind) in x.iter().zip(&columns[1]) {
        assert_eq!(kind.as_deref(), Some(if *value < 50.0 { "a" } else { "b" }));
    }
    // seeded
    assert_eq!(columns, generate(&sketch, 8000, 1));
    assert_ne!(columns, generate(&sketch, 8000, 2));
}

#[test]
fn nulls_and_integers_follow_the_axis() {
    let mut sketch = two_class_sketch();
    sketch.columns[0].kind = ColumnKind::Numeric { integer: true };
    sketch.classes.truncate(1);
    if let Axis::Numeric { null_share, .. } = &mut sketch.classes[0].axes[0] {
        *null_share = 0.25;
    }
    let columns = generate(&sketch, 8000, 3);
    let nulls = columns[0].iter().filter(|v| v.is_none()).count() as f64 / 8000.0;
    assert!((nulls - 0.25).abs() < 0.02, "{nulls}");
    assert!(columns[0]
        .iter()
        .flatten()
        .all(|v| v.parse::<i64>().is_ok_and(|i| (0..=10).contains(&i))));
}

// ---- steering: the output follows what was drawn

#[test]
fn narrowing_a_range_dropping_a_class_and_resizing_change_the_output_as_drawn() {
    // narrow the low class to 2..4
    let mut narrowed = two_class_sketch();
    if let Axis::Numeric { min, max, .. } = &mut narrowed.classes[0].axes[0] {
        (*min, *max) = (2.0, 4.0);
    }
    let x = numbers(&generate(&narrowed, 4000, 1), 0);
    assert!(x.iter().all(|v| (2.0..=4.0).contains(v) || *v >= 100.0));

    // drop the high class
    let mut dropped = two_class_sketch();
    dropped.classes.remove(1);
    let columns = generate(&dropped, 4000, 1);
    assert!(numbers(&columns, 0).iter().all(|v| *v <= 10.0));
    assert!(columns[1].iter().all(|v| v.as_deref() == Some("a")));

    // make the two classes the same size
    let mut resized = two_class_sketch();
    resized.classes[1].size = 300;
    let x = numbers(&generate(&resized, 8000, 1), 0);
    assert!((share_below(&x, 50.0) - 0.5).abs() < 0.02);
}

#[test]
fn merging_two_classes_adds_their_sizes_and_keeps_both_shapes() {
    let mut merged = two_class_sketch();
    merged.merge_classes(0, 1).unwrap();
    merged.validate().unwrap();
    assert_eq!(merged.classes.len(), 1);
    assert_eq!(merged.classes[0].size, 400);
    assert_eq!(merged.classes[0].name, "low");
    let Axis::Numeric { min, max, .. } = merged.classes[0].axes[0] else {
        panic!("a number axis")
    };
    assert_eq!((min, max), (0.0, 110.0));

    let columns = generate(&merged, 8000, 1);
    let x = numbers(&columns, 0);
    // the mass stays where the two classes had it: three quarters low, nothing in the gap
    assert!((share_below(&x, 55.0) - 0.75).abs() < 0.02);
    assert!(
        x.iter().all(|v| *v <= 13.75 || *v >= 96.25),
        "values in the gap"
    );
    // the category shares mix 3 to 1, and no longer follow the range: that is what merging loses
    let a = columns[1]
        .iter()
        .filter(|v| v.as_deref() == Some("a"))
        .count() as f64
        / 8000.0;
    assert!((a - 0.75).abs() < 0.02, "{a}");

    let mut same = two_class_sketch();
    assert!(same.merge_classes(1, 1).is_err());
    assert!(same.merge_classes(0, 7).is_err());
}

// ---- the report

#[test]
fn the_report_claims_no_privacy_and_says_the_sketch_is_derived_from_the_data() {
    let real = customers(800);
    let sketch = propose(&real, &config(3)).unwrap();
    let result = synthesize_sketch(&real, &sketch, &SynthesizeConfig::default()).unwrap();
    let report = &result.report;
    assert_eq!(report.method, "sketch");
    assert_eq!(result.sample.rows.len(), 800);
    assert!(report
        .privacy_statement
        .contains("no formal privacy guarantee"));
    assert!(report
        .limitations
        .iter()
        .any(|l| l.contains("handle the sketch like the data itself")));
    let summary = report.sketch.as_ref().unwrap();
    assert_eq!(summary.classes.len(), sketch.classes.len());
    assert_eq!(summary.classes.iter().map(|c| c.size).sum::<usize>(), 800);
    let json = serde_json::to_string(report).unwrap();
    assert!(json.contains("\"sketch\""));
    for word in ["anonymous data", "differentially private", "guaranteed"] {
        assert!(!json.to_lowercase().contains(word), "{word}");
    }
}

#[test]
fn a_sketch_must_fit_the_data_it_is_compared_with() {
    let real = customers(800);
    let mut sketch = propose(&real, &config(2)).unwrap();
    // a column removed from the sketch is reported as dropped, not an error
    let index = sketch
        .columns
        .iter()
        .position(|c| c.name == "score")
        .unwrap();
    sketch.columns.remove(index);
    sketch.classes.iter_mut().for_each(|c| {
        c.axes.remove(index);
    });
    let result = synthesize_sketch(&real, &sketch, &SynthesizeConfig::default()).unwrap();
    assert!(!result.sample.headers.contains(&"score".to_string()));
    assert!(result
        .report
        .columns_dropped
        .iter()
        .any(|d| d.column == "score" && d.reason == "not in the sketch"));

    // a column the data does not have is an error
    sketch.columns[0].name = "nope".into();
    let error = synthesize_sketch(&real, &sketch, &SynthesizeConfig::default())
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("\"nope\""), "{error}");
}
