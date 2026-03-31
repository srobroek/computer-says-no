# REST API Contract

**Base URL**: `http://127.0.0.1:{port}` (default port: 9847)

All endpoints return JSON. Error responses use HTTP status codes with a JSON body: `{"error": "message"}`.

## POST /classify

Classify text against a reference set.

**Request**:
```json
{"text": "string", "reference_set": "string"}
```

**Response (binary mode)** — 200:
```json
{
  "match": true,
  "confidence": 0.88,
  "top_phrase": "no, use X instead",
  "scores": {"positive": 0.88, "negative": 0.42}
}
```

**Response (multi-category mode)** — 200:
```json
{
  "match": true,
  "category": "feat",
  "confidence": 0.82,
  "top_phrase": "create new endpoint",
  "all_scores": [
    {"category": "feat", "score": 0.82, "top_phrase": "create new endpoint"},
    {"category": "fix", "score": 0.56, "top_phrase": "resolve issue"}
  ]
}
```

**Errors**:
- 404: reference set not found (body includes available set names)
- 500: embedding failure

## POST /embed

Generate embedding vector for text.

**Request**:
```json
{"text": "string"}
```

**Response** — 200:
```json
{"embedding": [0.023, -0.041, ...], "dimensions": 768, "model": "nomic-embed-text-v1.5-Q"}
```

## POST /similarity

Compute cosine similarity between two texts.

**Request**:
```json
{"a": "string", "b": "string"}
```

**Response** — 200:
```json
{"similarity": 0.73}
```

## GET /health

**Response** — 200:
```json
{"status": "ok", "model": "nomic-embed-text-v1.5-Q", "sets": 2, "uptime": "2h34m"}
```

## GET /sets

**Response** — 200:
```json
[
  {"name": "corrections", "phrases": 47, "mode": "binary"},
  {"name": "commit-types", "phrases": 36, "mode": "multi-category"}
]
```
