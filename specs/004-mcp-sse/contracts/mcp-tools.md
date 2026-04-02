# Contract: MCP Tools

## Transport

- Protocol: MCP (Model Context Protocol) over stdio (JSON-RPC via stdin/stdout)
- Server spawned by client: `{"command": "csn", "args": ["mcp"]}`
- Single-client, sequential tool calls (stdio constraint)

## Tools

### classify

Classify text against a reference set using combined pipeline (MLP + cosine).

**Parameters**:
```json
{
  "text": {"type": "string", "description": "Text to classify"},
  "reference_set": {"type": "string", "description": "Name of the reference set to classify against"}
}
```

**Result** (text content, JSON):
```json
{
  "match": true,
  "confidence": 0.97,
  "top_phrase": "revert that",
  "scores": {
    "positive": 0.78,
    "negative": 0.55
  }
}
```

**Errors**:
- Unknown reference set: lists available sets
- Missing parameters: describes required fields

### list_sets

List all loaded reference sets with metadata.

**Parameters**: none

**Result** (text content, JSON):
```json
[
  {"name": "corrections", "mode": "binary", "phrase_count": 1587}
]
```

### embed

Generate embedding vector for text.

**Parameters**:
```json
{
  "text": {"type": "string", "description": "Text to embed"}
}
```

**Result** (text content, JSON):
```json
{
  "embedding": [0.123, -0.456, ...],
  "dimensions": 384,
  "model": "bge-small-en-v1.5-Q"
}
```

### similarity

Compute cosine similarity between two texts.

**Parameters**:
```json
{
  "a": {"type": "string", "description": "First text"},
  "b": {"type": "string", "description": "Second text"}
}
```

**Result** (text content, JSON):
```json
{
  "similarity": 0.8234,
  "model": "bge-small-en-v1.5-Q"
}
```
