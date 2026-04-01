#!/usr/bin/env python3
"""
v2: Train classifiers using reference phrases + dataset split.
Uses 5-fold cross-validation for reliable accuracy.
"""

import json
import time
from pathlib import Path

import numpy as np
from fastembed import TextEmbedding
from sklearn.linear_model import LogisticRegression
from sklearn.svm import LinearSVC
from sklearn.calibration import CalibratedClassifierCV
from sklearn.neural_network import MLPClassifier
from sklearn.ensemble import GradientBoostingClassifier, RandomForestClassifier
from sklearn.model_selection import cross_val_score
from sklearn.metrics import accuracy_score


def load_reference_set(path):
    import tomllib
    with open(path, "rb") as f:
        return tomllib.load(f)


def load_dataset(path):
    with open(path) as f:
        return json.load(f)


def embed_texts(model, texts):
    return np.array(list(model.embed(texts)))


def cosine_similarity(a, b):
    a_norm = a / np.linalg.norm(a)
    b_norm = b / np.linalg.norm(b, axis=1, keepdims=True)
    return a_norm @ b_norm.T


def main():
    model_name = "BAAI/bge-small-en-v1.5"
    print(f"Model: {model_name}\n")

    ref_set = load_reference_set(Path("reference-sets/corrections.toml"))
    dataset = load_dataset(Path("datasets/pushback.json"))

    print(f"Loading embedding model...")
    model = TextEmbedding(model_name=model_name)

    # === Cosine baseline ===
    pos_phrases = ref_set["phrases"]["positive"]
    neg_phrases = ref_set["phrases"].get("negative", [])
    print(f"\nEmbedding {len(pos_phrases)} pos + {len(neg_phrases)} neg reference phrases...")
    pos_emb = embed_texts(model, pos_phrases)
    neg_emb = embed_texts(model, neg_phrases)

    test_texts = [p["text"] for p in dataset["prompts"]]
    test_labels = [1 if p["expected_label"] == "match" else 0 for p in dataset["prompts"]]
    print(f"Embedding {len(test_texts)} test prompts...")
    test_emb = embed_texts(model, test_texts)

    # Cosine + margin baseline
    cos_preds = []
    for emb in test_emb:
        pos_sim = cosine_similarity(emb, pos_emb).max()
        neg_sim = cosine_similarity(emb, neg_emb).max() if len(neg_emb) > 0 else 0
        cos_preds.append(1 if (pos_sim - neg_sim) > 0.05 else 0)
    cos_acc = accuracy_score(test_labels, cos_preds)
    print(f"\n{'='*50}")
    print(f"Cosine + margin-0.05:  {cos_acc:.1%}")
    print(f"{'='*50}")

    # === Approach 1: Train on reference phrases only ===
    print(f"\n--- Approach 1: Train on reference phrases only ---")
    ref_texts = pos_phrases + neg_phrases
    ref_labels = [1]*len(pos_phrases) + [0]*len(neg_phrases)
    ref_emb = np.vstack([pos_emb, neg_emb])

    classifiers_1 = {
        "LogReg": LogisticRegression(max_iter=1000, C=1.0, class_weight="balanced"),
        "LogReg-C10": LogisticRegression(max_iter=1000, C=10.0, class_weight="balanced"),
        "SVM": CalibratedClassifierCV(LinearSVC(C=1.0, class_weight="balanced", max_iter=2000)),
        "MLP(64)": MLPClassifier(hidden_layer_sizes=(64,), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "MLP(128,64)": MLPClassifier(hidden_layer_sizes=(128, 64), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "GBM-small": GradientBoostingClassifier(n_estimators=30, max_depth=2, learning_rate=0.3, random_state=42),
        "RF": RandomForestClassifier(n_estimators=100, max_depth=10, class_weight="balanced", random_state=42),
    }

    for name, clf in classifiers_1.items():
        t0 = time.time()
        clf.fit(ref_emb, ref_labels)
        ms = (time.time() - t0) * 1000
        preds = clf.predict(test_emb)
        acc = accuracy_score(test_labels, preds)
        print(f"  {name:>15}: {acc:.1%}  ({ms:.0f}ms)")

    # === Approach 2: Train on reference phrases + dataset (5-fold CV) ===
    print(f"\n--- Approach 2: Combined training with 5-fold CV ---")
    combined_emb = np.vstack([ref_emb, test_emb])
    combined_labels = np.array(ref_labels + test_labels)
    print(f"  Combined: {len(combined_labels)} samples ({sum(combined_labels)} pos, {len(combined_labels)-sum(combined_labels)} neg)")

    classifiers_2 = {
        "LogReg": LogisticRegression(max_iter=1000, C=1.0, class_weight="balanced"),
        "LogReg-C10": LogisticRegression(max_iter=1000, C=10.0, class_weight="balanced"),
        "SVM": LinearSVC(C=1.0, class_weight="balanced", max_iter=2000),
        "MLP(64)": MLPClassifier(hidden_layer_sizes=(64,), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "MLP(128,64)": MLPClassifier(hidden_layer_sizes=(128, 64), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "GBM-small": GradientBoostingClassifier(n_estimators=30, max_depth=2, learning_rate=0.3, random_state=42),
        "RF": RandomForestClassifier(n_estimators=100, max_depth=10, class_weight="balanced", random_state=42),
    }

    for name, clf in classifiers_2.items():
        scores = cross_val_score(clf, combined_emb, combined_labels, cv=5, scoring="accuracy")
        print(f"  {name:>15}: {scores.mean():.1%} ± {scores.std():.1%}")

    # === Approach 3: Train on dataset only (no reference phrases) ===
    print(f"\n--- Approach 3: Train on dataset only (5-fold CV) ---")
    classifiers_3 = {
        "LogReg": LogisticRegression(max_iter=1000, C=1.0, class_weight="balanced"),
        "SVM": LinearSVC(C=1.0, class_weight="balanced", max_iter=2000),
        "MLP(64)": MLPClassifier(hidden_layer_sizes=(64,), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "MLP(128,64)": MLPClassifier(hidden_layer_sizes=(128, 64), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "GBM-small": GradientBoostingClassifier(n_estimators=30, max_depth=2, learning_rate=0.3, random_state=42),
    }

    for name, clf in classifiers_3.items():
        scores = cross_val_score(clf, test_emb, np.array(test_labels), cv=5, scoring="accuracy")
        print(f"  {name:>15}: {scores.mean():.1%} ± {scores.std():.1%}")

    # === Approach 4: Mix everything, 80/20 split ===
    print(f"\n--- Approach 4: Mixed 80/20 split (10 random seeds) ---")
    from sklearn.model_selection import train_test_split

    best_classifiers = {
        "LogReg": lambda: LogisticRegression(max_iter=1000, C=1.0, class_weight="balanced"),
        "MLP(128,64)": lambda: MLPClassifier(hidden_layer_sizes=(128, 64), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "GBM": lambda: GradientBoostingClassifier(n_estimators=30, max_depth=2, learning_rate=0.3, random_state=42),
    }

    for name, make_clf in best_classifiers.items():
        accs = []
        for seed in range(10):
            X_train, X_test, y_train, y_test = train_test_split(
                combined_emb, combined_labels, test_size=0.2, random_state=seed, stratify=combined_labels
            )
            clf = make_clf()
            clf.fit(X_train, y_train)
            acc = accuracy_score(y_test, clf.predict(X_test))
            accs.append(acc)
        print(f"  {name:>15}: {np.mean(accs):.1%} ± {np.std(accs):.1%}  (min {np.min(accs):.1%}, max {np.max(accs):.1%})")

    print(f"\n{'='*50}")
    print(f"Cosine baseline:      {cos_acc:.1%}")
    print(f"{'='*50}")


if __name__ == "__main__":
    main()
