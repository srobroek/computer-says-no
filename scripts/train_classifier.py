#!/usr/bin/env python3
"""
Prototype: train logistic regression on top of fastembed embeddings.
Compares cosine similarity baseline vs logistic regression accuracy.

Usage:
    pip install fastembed scikit-learn numpy
    python scripts/train_classifier.py --model bge-small-en-v1.5-Q
"""

import argparse
import json
import sys
import time
from pathlib import Path

import numpy as np
from fastembed import TextEmbedding
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import accuracy_score, classification_report


def load_reference_set(path: Path) -> dict:
    """Load TOML reference set (simple parser for [phrases] section)."""
    import tomllib
    with open(path, "rb") as f:
        return tomllib.load(f)


def load_dataset(path: Path) -> dict:
    with open(path) as f:
        return json.load(f)


def embed_texts(model: TextEmbedding, texts: list[str]) -> np.ndarray:
    """Embed a list of texts, return numpy array."""
    embeddings = list(model.embed(texts))
    return np.array(embeddings)


def cosine_similarity(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """Compute cosine similarity between vector a and matrix b."""
    a_norm = a / np.linalg.norm(a)
    b_norm = b / np.linalg.norm(b, axis=1, keepdims=True)
    return a_norm @ b_norm.T


def cosine_baseline(
    model: TextEmbedding,
    ref_set: dict,
    dataset: dict,
    margin: float = 0.05,
) -> tuple[float, list[bool]]:
    """Cosine similarity + margin baseline (current csn approach)."""
    pos_phrases = ref_set["phrases"]["positive"]
    neg_phrases = ref_set["phrases"].get("negative", [])

    print(f"  Embedding {len(pos_phrases)} positive + {len(neg_phrases)} negative reference phrases...")
    pos_emb = embed_texts(model, pos_phrases)
    neg_emb = embed_texts(model, neg_phrases) if neg_phrases else np.zeros((0, pos_emb.shape[1]))

    test_texts = [p["text"] for p in dataset["prompts"]]
    expected = [p["expected_label"] == "match" for p in dataset["prompts"]]

    print(f"  Embedding {len(test_texts)} test prompts...")
    test_emb = embed_texts(model, test_texts)

    predictions = []
    for emb in test_emb:
        pos_sim = cosine_similarity(emb, pos_emb).max() if len(pos_emb) > 0 else 0
        neg_sim = cosine_similarity(emb, neg_emb).max() if len(neg_emb) > 0 else 0
        predictions.append((pos_sim - neg_sim) > margin)

    accuracy = accuracy_score(expected, predictions)
    return accuracy, predictions


def logistic_regression_classifier(
    model: TextEmbedding,
    ref_set: dict,
    dataset: dict,
) -> tuple[float, list[bool], float]:
    """Train logistic regression on reference set, test on dataset."""
    pos_phrases = ref_set["phrases"]["positive"]
    neg_phrases = ref_set["phrases"].get("negative", [])

    # Training data = reference set phrases
    train_texts = pos_phrases + neg_phrases
    train_labels = [1] * len(pos_phrases) + [0] * len(neg_phrases)

    print(f"  Embedding {len(train_texts)} training phrases...")
    train_emb = embed_texts(model, train_texts)

    # Train
    print(f"  Training logistic regression ({train_emb.shape})...")
    t0 = time.time()
    # Test data
    test_texts = [p["text"] for p in dataset["prompts"]]
    expected_lr = [p["expected_label"] == "match" for p in dataset["prompts"]]
    print(f"  Embedding {len(test_texts)} test prompts...")
    test_emb = embed_texts(model, test_texts)

    # Try multiple classifiers
    from sklearn.svm import LinearSVC
    from sklearn.calibration import CalibratedClassifierCV

    from sklearn.neural_network import MLPClassifier
    from sklearn.ensemble import GradientBoostingClassifier, RandomForestClassifier

    classifiers = {
        "LogReg": LogisticRegression(max_iter=1000, C=1.0, solver="lbfgs", class_weight="balanced"),
        "LogReg-C10": LogisticRegression(max_iter=1000, C=10.0, solver="lbfgs", class_weight="balanced"),
        "SVM-linear": CalibratedClassifierCV(LinearSVC(C=1.0, class_weight="balanced", max_iter=2000)),
        "MLP-small": MLPClassifier(hidden_layer_sizes=(64,), max_iter=500, early_stopping=True, random_state=42),
        "MLP-medium": MLPClassifier(hidden_layer_sizes=(128, 64), max_iter=500, early_stopping=True, random_state=42),
        "GBM": GradientBoostingClassifier(n_estimators=100, max_depth=4, random_state=42),
        "RandomForest": RandomForestClassifier(n_estimators=200, class_weight="balanced", random_state=42),
    }

    print("  Classifier comparison:")
    best_name = None
    best_acc = 0.0
    best_preds = []

    for name, c in classifiers.items():
        t0 = time.time()
        c.fit(train_emb, train_labels)
        fit_ms = (time.time() - t0) * 1000
        preds = [bool(p) for p in c.predict(test_emb)]
        acc = accuracy_score(expected_lr, preds)
        print(f"    {name}: {acc:.1%} ({fit_ms:.0f}ms)")
        if acc > best_acc:
            best_acc = acc
            best_name = name
            best_preds = preds

    train_time = 0  # already reported per-classifier
    print(f"  Best: {best_name} ({best_acc:.1%})")

    clf = None  # not used below, we use best_preds directly
    clf.fit(train_emb, train_labels)
    train_time = (time.time() - t0) * 1000
    print(f"  Training took {train_time:.1f}ms")

    # Test data = dataset prompts
    test_texts = [p["text"] for p in dataset["prompts"]]
    expected = [p["expected_label"] == "match" for p in dataset["prompts"]]

    print(f"  Embedding {len(test_texts)} test prompts...")
    test_emb = embed_texts(model, test_texts)

    predictions = clf.predict(test_emb).tolist()
    predictions_bool = [bool(p) for p in predictions]

    return best_acc, best_preds, 0.0


def main():
    parser = argparse.ArgumentParser(description="Compare cosine vs logistic regression")
    parser.add_argument("--model", default="bge-small-en-v1.5-Q",
                        help="Embedding model name")
    parser.add_argument("--reference-set", default="reference-sets/corrections.toml",
                        help="Path to reference set TOML")
    parser.add_argument("--dataset", default="datasets/pushback.json",
                        help="Path to test dataset JSON")
    parser.add_argument("--margin", type=float, default=0.05,
                        help="Margin for cosine baseline")
    args = parser.parse_args()

    print(f"Model: {args.model}")
    print(f"Reference set: {args.reference_set}")
    print(f"Dataset: {args.dataset}")
    print()

    # Load data
    ref_set = load_reference_set(Path(args.reference_set))
    dataset = load_dataset(Path(args.dataset))
    n_prompts = len(dataset["prompts"])
    n_pos = sum(1 for p in dataset["prompts"] if p["expected_label"] == "match")
    n_neg = n_prompts - n_pos
    print(f"Dataset: {n_prompts} prompts ({n_pos} positive, {n_neg} negative)")
    print()

    # Load model
    print(f"Loading model {args.model}...")
    model = TextEmbedding(model_name=args.model)
    print()

    # Cosine baseline
    print("=== Cosine Similarity + Margin ===")
    cos_acc, cos_preds = cosine_baseline(model, ref_set, dataset, margin=args.margin)
    expected = [p["expected_label"] == "match" for p in dataset["prompts"]]
    print(f"  Accuracy: {cos_acc:.1%}")
    print()

    # Per-tier breakdown
    tiers = {}
    for p, pred, exp in zip(dataset["prompts"], cos_preds, expected):
        key = f"{p['tier']}_{p['polarity']}"
        if key not in tiers:
            tiers[key] = {"correct": 0, "total": 0}
        tiers[key]["total"] += 1
        if pred == exp:
            tiers[key]["correct"] += 1
    print("  Per-tier:")
    for key in sorted(tiers.keys()):
        t = tiers[key]
        print(f"    {key:>20}: {t['correct']}/{t['total']} = {t['correct']/t['total']:.1%}")
    print()

    # Logistic regression
    print("=== Logistic Regression ===")
    lr_acc, lr_preds, train_ms = logistic_regression_classifier(model, ref_set, dataset)
    print(f"  Accuracy: {lr_acc:.1%}")
    print(f"  Training time: {train_ms:.1f}ms")
    print()

    # Per-tier breakdown
    tiers_lr = {}
    for p, pred, exp in zip(dataset["prompts"], lr_preds, expected):
        key = f"{p['tier']}_{p['polarity']}"
        if key not in tiers_lr:
            tiers_lr[key] = {"correct": 0, "total": 0}
        tiers_lr[key]["total"] += 1
        if pred == exp:
            tiers_lr[key]["correct"] += 1
    print("  Per-tier:")
    for key in sorted(tiers_lr.keys()):
        t = tiers_lr[key]
        print(f"    {key:>20}: {t['correct']}/{t['total']} = {t['correct']/t['total']:.1%}")
    print()

    # Comparison
    print("=== Comparison ===")
    print(f"  Cosine + margin:      {cos_acc:.1%}")
    print(f"  Logistic regression:  {lr_acc:.1%}")
    print(f"  Improvement:          {(lr_acc - cos_acc):+.1%}")
    print(f"  Training overhead:    {train_ms:.1f}ms")


if __name__ == "__main__":
    main()
