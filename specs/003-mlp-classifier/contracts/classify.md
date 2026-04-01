# Contract: /classify Endpoint (MLP Enhancement)

## Request (unchanged)

```json
POST /classify
{
  "text": "string",
  "reference_set": "string"
}
```

No new fields. The combined pipeline activates automatically for binary sets with a trained MLP.

## Response (unchanged shape, new confidence semantics)

### When MLP is active (binary set with trained model)

```json
{
  "match": true,
  "confidence": 0.92,
  "top_phrase": "that's not what I asked",
  "scores": {
    "positive": 0.78,
    "negative": 0.31
  }
}
```

- `confidence`: MLP sigmoid probability (0.0-1.0), NOT cosine similarity
- `scores.positive`: max cosine similarity to positive phrases (raw, for transparency)
- `scores.negative`: max cosine similarity to negative phrases (raw, for transparency)
- `match`: `confidence > 0.5`
- `top_phrase`: phrase with highest positive cosine similarity (unchanged)

### When MLP is not available (fallback to pure cosine)

```json
{
  "match": true,
  "confidence": 0.78,
  "top_phrase": "that's not what I asked",
  "scores": {
    "positive": 0.78,
    "negative": 0.31
  }
}
```

- `confidence`: max positive cosine similarity (existing behavior)
- No indication in response that MLP is absent (transparent fallback)

## Backward Compatibility

- Request schema: unchanged
- Response schema: unchanged
- Behavioral change: `confidence` value may differ (MLP probability vs cosine score)
- `scores.positive`/`scores.negative`: always raw cosine values regardless of pipeline
