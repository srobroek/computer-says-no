# Quickstart: MCP Server + Architecture Simplification

## For MCP Clients (Claude Code, Cursor)

Add to your MCP configuration:

```json
{
  "mcpServers": {
    "csn": {
      "command": "csn",
      "args": ["mcp"]
    }
  }
}
```

Tools available: `classify`, `list_sets`, `embed`, `similarity`.

## For CLI Usage

```bash
# Classify text (in-process, no daemon needed)
csn classify "that's completely wrong" --set corrections

# Embed text
csn embed "hello world"

# Compare two texts
csn similarity "revert that" "good job"

# List reference sets
csn sets list

# List models
csn models

# Run benchmarks
csn benchmark run
csn benchmark compare-strategies --dataset pushback
```

## What Changed from Previous Versions

- `csn serve` removed — no daemon needed
- `--standalone` flag removed — everything runs in-process by default
- `csn mcp` added — stdio MCP server for AI agent integration
- Hot-reload removed — restart `csn mcp` to pick up reference set changes
