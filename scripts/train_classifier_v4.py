#!/usr/bin/env python3
"""
v4: Ensemble cosine similarity + MLP classifier.
Tests different combination strategies.
"""

import json
from pathlib import Path

import numpy as np
from fastembed import TextEmbedding
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


def cosine_scores(emb, pos_emb, neg_emb):
    """Return (pos_best_sim, neg_best_sim, margin) for a single embedding."""
    a_norm = emb / np.linalg.norm(emb)
    pos_sims = a_norm @ (pos_emb / np.linalg.norm(pos_emb, axis=1, keepdims=True)).T
    neg_sims = a_norm @ (neg_emb / np.linalg.norm(neg_emb, axis=1, keepdims=True)).T if len(neg_emb) > 0 else np.array([0])
    return pos_sims.max(), neg_sims.max(), pos_sims.max() - neg_sims.max()


def main():
    model_name = "BAAI/bge-small-en-v1.5"
    print(f"Model: {model_name}\n")

    ref_set = load_toml(Path("reference-sets/corrections.toml"))
    dataset = load_json(Path("datasets/pushback.json"))

    model = TextEmbedding(model_name=model_name)

    pos_phrases = ref_set["phrases"]["positive"]
    neg_phrases = ref_set["phrases"].get("negative", [])
    print(f"Embedding {len(pos_phrases)} pos + {len(neg_phrases)} neg phrases...")
    pos_emb = embed(model, pos_phrases)
    neg_emb = embed(model, neg_phrases)

    test_texts = [p["text"] for p in dataset["prompts"]]
    test_labels = np.array([1 if p["expected_label"] == "match" else 0 for p in dataset["prompts"]])
    print(f"Embedding {len(test_texts)} test prompts...")
    test_emb = embed(model, test_texts)

    # Compute cosine features for all samples
    ref_emb = np.vstack([pos_emb, neg_emb])
    ref_labels = np.array([1]*len(pos_phrases) + [0]*len(neg_phrases))

    print("Computing cosine features...")
    all_emb = np.vstack([ref_emb, test_emb])
    all_labels = np.concatenate([ref_labels, test_labels])

    cos_features = []
    for emb in all_emb:
        pos_sim, neg_sim, margin = cosine_scores(emb, pos_emb, neg_emb)
        cos_features.append([pos_sim, neg_sim, margin])
    cos_features = np.array(cos_features)

    # === Strategy 1: Cosine only (margin baseline) ===
    cos_preds_all = (cos_features[:, 2] > 0.05).astype(int)
    # Only test portion
    n_ref = len(ref_labels)
    cos_test_acc = accuracy_score(test_labels, cos_preds_all[n_ref:])
    print(f"\nBaseline cosine + margin-0.05: {cos_test_acc:.1%}")

    # === Strategy 2: MLP on embeddings only ===
    # === Strategy 3: MLP on embeddings + cosine features ===
    # === Strategy 4: MLP on cosine features only ===
    # === Strategy 5: Voting ensemble (cosine + MLP, majority) ===
    # === Strategy 6: Stacking (MLP probability + cosine margin → LogReg) ===

    strategies = {}

    # Embedding-only features
    emb_features = all_emb

    # Embedding + cosine features (concatenated)
    combined_features = np.hstack([all_emb, cos_features])

    # Cosine features only (3 dims)
    cos_only_features = cos_features

    feature_sets = {
        "MLP(emb only)": emb_features,
        "MLP(emb+cosine)": combined_features,
        "MLP(cosine only)": cos_only_features,
    }

    print(f"\n{'='*60}")
    print(f"80/20 split, 10 seeds:")
    print(f"{'='*60}")

    for feat_name, features in feature_sets.items():
        accs = []
        for seed in range(10):
            X_train, X_test, y_train, y_test = train_test_split(
                features, all_labels, test_size=0.2, random_state=seed, stratify=all_labels
            )
            clf = MLPClassifier(hidden_layer_sizes=(256, 128), max_iter=500, alpha=0.01,
                                early_stopping=True, random_state=42)
            clf.fit(X_train, y_train)
            accs.append(accuracy_score(y_test, clf.predict(X_test)))
        print(f"  {feat_name:>25}: {np.mean(accs):.1%} ± {np.std(accs):.1%}  "
              f"(min {np.min(accs):.1%}, max {np.max(accs):.1%})")

    # === Voting ensemble ===
    print(f"\n{'='*60}")
    print(f"Voting ensemble (cosine + MLP):")
    print(f"{'='*60}")

    accs_vote = []
    accs_avg = []
    for seed in range(10):
        X_train, X_test, y_train, y_test = train_test_split(
            all_emb, all_labels, test_size=0.2, random_state=seed, stratify=all_labels
        )
        cos_train, cos_test = train_test_split(
            cos_features, all_labels, test_size=0.2, random_state=seed, stratify=all_labels
        )

        # Train MLP on embeddings
        mlp = MLPClassifier(hidden_layer_sizes=(256, 128), max_iter=500, alpha=0.01,
                            early_stopping=True, random_state=42)
        mlp.fit(X_train, y_train)
        mlp_proba = mlp.predict_proba(X_test)[:, 1]
        mlp_preds = (mlp_proba > 0.5).astype(int)

        # Cosine predictions
        cos_preds = (cos_test[0][:, 2] > 0.05).astype(int)

        # Voting: both agree → that label, disagree → use MLP confidence
        vote_preds = np.where(mlp_preds == cos_preds, mlp_preds,
                              (mlp_proba > 0.5).astype(int))
        accs_vote.append(accuracy_score(y_test, vote_preds))

        # Average probability: (mlp_prob + cosine_signal) / 2
        cos_signal = (cos_test[0][:, 2] + 0.5).clip(0, 1)  # normalize margin to 0-1 range
        avg_prob = (mlp_proba + cos_signal) / 2
        avg_preds = (avg_prob > 0.5).astype(int)
        accs_avg.append(accuracy_score(y_test, avg_preds))

    print(f"  {'Vote (MLP tiebreak)':>25}: {np.mean(accs_vote):.1%} ± {np.std(accs_vote):.1%}")
    print(f"  {'Avg probability':>25}: {np.mean(accs_avg):.1%} ± {np.std(accs_avg):.1%}")

    # === Stacking: cosine features + MLP probability → final LogReg ===
    print(f"\n{'='*60}")
    print(f"Stacking (MLP proba + cosine → LogReg):")
    print(f"{'='*60}")

    from sklearn.linear_model import LogisticRegression

    accs_stack = []
    for seed in range(10):
        # Outer split
        idx = np.arange(len(all_labels))
        idx_train, idx_test = train_test_split(idx, test_size=0.2, random_state=seed,
                                                stratify=all_labels)

        # Train MLP on training embeddings
        mlp = MLPClassifier(hidden_layer_sizes=(256, 128), max_iter=500, alpha=0.01,
                            early_stopping=True, random_state=42)
        mlp.fit(all_emb[idx_train], all_labels[idx_train])

        # Get MLP probabilities for test set
        mlp_test_proba = mlp.predict_proba(all_emb[idx_test])[:, 1].reshape(-1, 1)

        # Combine with cosine features
        stack_test = np.hstack([mlp_test_proba, cos_features[idx_test]])

        # Get MLP probabilities for train set (use cross-val to avoid leakage)
        from sklearn.model_selection import cross_val_predict
        mlp_train_proba = cross_val_predict(
            MLPClassifier(hidden_layer_sizes=(256, 128), max_iter=500, alpha=0.01,
                          early_stopping=True, random_state=42),
            all_emb[idx_train], all_labels[idx_train], cv=3, method="predict_proba"
        )[:, 1].reshape(-1, 1)

        stack_train = np.hstack([mlp_train_proba, cos_features[idx_train]])

        # Train stacking LogReg
        stack_clf = LogisticRegression(max_iter=1000, class_weight="balanced")
        stack_clf.fit(stack_train, all_labels[idx_train])
        stack_preds = stack_clf.predict(stack_test)
        accs_stack.append(accuracy_score(all_labels[idx_test], stack_preds))

    print(f"  {'Stacked':>25}: {np.mean(accs_stack):.1%} ± {np.std(accs_stack):.1%}  "
          f"(min {np.min(accs_stack):.1%}, max {np.max(accs_stack):.1%})")

    print(f"\n{'='*60}")
    print(f"Summary (bge-small-en-v1.5):")
    print(f"  Cosine baseline:   {cos_test_acc:.1%}")
    print(f"{'='*60}")


if __name__ == "__main__":
    main()
