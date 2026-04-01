#!/usr/bin/env python3
"""
v3: Test MLP classifier across multiple embedding models.
Mixed 80/20 split with 10 random seeds for reliable estimates.
"""

import json
import time
from pathlib import Path

import numpy as np
from fastembed import TextEmbedding
from sklearn.linear_model import LogisticRegression
from sklearn.neural_network import MLPClassifier
from sklearn.model_selection import train_test_split
from sklearn.metrics import accuracy_score


def load_toml(path):
    import tomllib
    with open(path, "rb") as f:
        return tomllib.load(f)


def load_json(path):
    with open(path) as f:
        return json.load(f)


def embed(model, texts):
    return np.array(list(model.embed(texts)))


def cosine_baseline(pos_emb, neg_emb, test_emb, margin=0.05):
    preds = []
    for emb in test_emb:
        a_norm = emb / np.linalg.norm(emb)
        pos_sim = (a_norm @ (pos_emb / np.linalg.norm(pos_emb, axis=1, keepdims=True)).T).max()
        neg_sim = (a_norm @ (neg_emb / np.linalg.norm(neg_emb, axis=1, keepdims=True)).T).max() if len(neg_emb) > 0 else 0
        preds.append(1 if (pos_sim - neg_sim) > margin else 0)
    return preds


def run_model(model_name):
    print(f"\n{'='*60}")
    print(f"Model: {model_name}")
    print(f"{'='*60}")

    ref_set = load_toml(Path("reference-sets/corrections.toml"))
    dataset = load_json(Path("datasets/pushback.json"))

    model = TextEmbedding(model_name=model_name)

    # Embed reference phrases
    pos_phrases = ref_set["phrases"]["positive"]
    neg_phrases = ref_set["phrases"].get("negative", [])
    print(f"Embedding {len(pos_phrases)} pos + {len(neg_phrases)} neg phrases...")
    t0 = time.time()
    pos_emb = embed(model, pos_phrases)
    neg_emb = embed(model, neg_phrases)
    embed_ref_ms = (time.time() - t0) * 1000
    dims = pos_emb.shape[1]
    print(f"  {dims} dims, {embed_ref_ms:.0f}ms")

    # Embed test data
    test_texts = [p["text"] for p in dataset["prompts"]]
    test_labels = np.array([1 if p["expected_label"] == "match" else 0 for p in dataset["prompts"]])
    print(f"Embedding {len(test_texts)} test prompts...")
    test_emb = embed(model, test_texts)

    # Measure single-text embed latency
    latencies = []
    for text in test_texts[:20]:
        t0 = time.time()
        embed(model, [text])
        latencies.append((time.time() - t0) * 1000)
    p50_embed = sorted(latencies)[len(latencies)//2]
    print(f"  Single embed p50: {p50_embed:.1f}ms")

    # === Cosine baseline ===
    cos_preds = cosine_baseline(pos_emb, neg_emb, test_emb, margin=0.05)
    cos_acc = accuracy_score(test_labels, cos_preds)
    print(f"\nCosine + margin-0.05: {cos_acc:.1%}")

    # === Combined training data ===
    ref_emb = np.vstack([pos_emb, neg_emb])
    ref_labels = np.array([1]*len(pos_phrases) + [0]*len(neg_phrases))
    combined_emb = np.vstack([ref_emb, test_emb])
    combined_labels = np.concatenate([ref_labels, test_labels])

    classifiers = {
        "LogReg": lambda: LogisticRegression(max_iter=1000, C=1.0, class_weight="balanced"),
        "MLP(64)": lambda: MLPClassifier(hidden_layer_sizes=(64,), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "MLP(128,64)": lambda: MLPClassifier(hidden_layer_sizes=(128, 64), max_iter=500, alpha=0.01, early_stopping=True, random_state=42),
        "MLP(256,128)": lambda: MLPClassifier(hidden_layer_sizes=(256, 128), max_iter=500, alpha=0.001, early_stopping=True, random_state=42),
    }

    print(f"\nMixed 80/20 split (10 seeds, {len(combined_labels)} samples):")
    for name, make_clf in classifiers.items():
        accs = []
        train_times = []
        infer_times = []
        for seed in range(10):
            X_train, X_test, y_train, y_test = train_test_split(
                combined_emb, combined_labels, test_size=0.2, random_state=seed, stratify=combined_labels
            )
            clf = make_clf()
            t0 = time.time()
            clf.fit(X_train, y_train)
            train_times.append((time.time() - t0) * 1000)

            t0 = time.time()
            preds = clf.predict(X_test)
            infer_times.append((time.time() - t0) * 1000 / len(X_test))

            accs.append(accuracy_score(y_test, preds))

        print(f"  {name:>15}: {np.mean(accs):.1%} ± {np.std(accs):.1%}  "
              f"(min {np.min(accs):.1%}, max {np.max(accs):.1%})  "
              f"train: {np.mean(train_times):.0f}ms  "
              f"infer: {np.mean(infer_times)*1000:.1f}µs/sample")

    print(f"\n  Total classify latency = embed ({p50_embed:.1f}ms) + infer (~0ms) = {p50_embed:.1f}ms")


def main():
    models = [
        "BAAI/bge-small-en-v1.5",
        "BAAI/bge-large-en-v1.5",
        "sentence-transformers/all-MiniLM-L6-v2",
        "nomic-ai/nomic-embed-text-v1.5",
    ]

    for m in models:
        try:
            run_model(m)
        except Exception as e:
            print(f"  ERROR: {e}")


if __name__ == "__main__":
    main()
