#!/usr/bin/env python3
"""Generates the ModelCard evaluation fixture and its golden values from independent oracles.

scikit-learn computes per-slice accuracy/precision/recall/F1/ROC-AUC; Fairlearn computes the
fairness metrics. Neither is a dependency of Triplex: they are used once, here, to produce
`tests/fixtures/modelcard/eval.golden.json`, which the Rust tests compare against. Rerun with:

    pip install scikit-learn fairlearn numpy pandas
    python scripts/modelcard_oracle.py

Synthetic data only (seeded), per the project guidelines.
"""
import json
import pathlib

import numpy as np
import pandas as pd
from fairlearn.metrics import (
    MetricFrame,
    demographic_parity_difference,
    demographic_parity_ratio,
    equalized_odds_difference,
    equalized_odds_ratio,
    false_positive_rate,
    selection_rate,
    true_positive_rate,
)
from sklearn.metrics import accuracy_score, f1_score, precision_score, recall_score, roc_auc_score

MIN_SLICE = 30
OUT = pathlib.Path(__file__).resolve().parent.parent / "tests" / "fixtures" / "modelcard"


def make_data(seed=7):
    rng = np.random.default_rng(seed)
    # group sizes chosen so one group ("islands") falls below MIN_SLICE and is suppressed
    sizes = {"north": 260, "south": 200, "east": 120, "islands": 20}
    rows = []
    base = {"north": 0.55, "south": 0.35, "east": 0.45, "islands": 0.5}
    shift = {"north": 0.0, "south": -0.10, "east": 0.05, "islands": 0.0}
    for region, n in sizes.items():
        label = rng.random(n) < base[region]
        # score: noisy signal, shifted per group so the model treats groups differently
        score = np.clip(0.25 + 0.5 * label + rng.normal(0, 0.22, n) + shift[region], 0.001, 0.999)
        pred = score >= 0.5
        tier = rng.choice(["basic", "pro"], n, p=[0.6, 0.4])
        for i in range(n):
            rows.append((int(label[i]), int(pred[i]), round(float(score[i]), 6), region, tier[i]))
    df = pd.DataFrame(rows, columns=["label", "prediction", "score", "region", "tier"])
    return df.sample(frac=1.0, random_state=1).reset_index(drop=True)


def slice_metrics(g):
    y, p, s = g["label"], g["prediction"], g["score"]
    out = {
        "n": int(len(g)),
        "selection_rate": float(p.mean()),
        "base_rate": float(y.mean()),
        "accuracy": float(accuracy_score(y, p)),
        "precision": float(precision_score(y, p)) if p.sum() else None,
        "recall": float(recall_score(y, p)) if y.sum() else None,
        "fpr": float(false_positive_rate(y, p)) if (y == 0).sum() else None,
        "f1": float(f1_score(y, p)) if (p.sum() + y.sum()) else None,
        "roc_auc": float(roc_auc_score(y, s)) if y.nunique() == 2 else None,
    }
    return out


def main():
    df = make_data()
    OUT.mkdir(parents=True, exist_ok=True)
    df.to_csv(OUT / "eval.csv", index=False)

    golden = {"min_slice_size": MIN_SLICE, "overall": slice_metrics(df), "slices": {}}
    for col in ["region", "tier"]:
        golden["slices"][col] = {
            str(k): slice_metrics(g) for k, g in df.groupby(col) if len(g) >= MIN_SLICE
        }
        golden["slices"][col].update(
            {str(k): {"n": int(len(g)), "suppressed": True} for k, g in df.groupby(col) if len(g) < MIN_SLICE}
        )

    keep = df.groupby("region")["label"].transform("size") >= MIN_SLICE
    d = df[keep]
    y, p, grp = d["label"], d["prediction"], d["region"]
    mf = MetricFrame(
        metrics={"tpr": true_positive_rate, "fpr": false_positive_rate, "sel": selection_rate},
        y_true=y, y_pred=p, sensitive_features=grp,
    )
    golden["fairness"] = {
        "protected_attribute": "region",
        "groups_compared": sorted(mf.by_group.index.tolist()),
        "demographic_parity_difference": float(demographic_parity_difference(y, p, sensitive_features=grp)),
        "demographic_parity_ratio": float(demographic_parity_ratio(y, p, sensitive_features=grp)),
        "equalized_odds_difference": float(equalized_odds_difference(y, p, sensitive_features=grp)),
        "equalized_odds_ratio": float(equalized_odds_ratio(y, p, sensitive_features=grp)),
        "tpr_by_group": {k: float(v) for k, v in mf.by_group["tpr"].items()},
        "fpr_by_group": {k: float(v) for k, v in mf.by_group["fpr"].items()},
    }
    (OUT / "eval.golden.json").write_text(json.dumps(golden, indent=2, sort_keys=True) + "\n")
    print("wrote", OUT / "eval.csv", "and eval.golden.json;", len(df), "rows")


if __name__ == "__main__":
    main()
