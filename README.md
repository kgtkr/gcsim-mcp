# gcsim-mcp

A [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server for [gcsim](https://gcsim.app/) (Genshin Impact combat simulator). It enables LLMs (such as Claude Desktop, Cursor, and AI agents) to validate configuration scripts (GCSL), run combat simulation samples, inspect DPS and team metrics, and query game reference identifiers.

## Public Demo Server

A public sample MCP server is available for testing:

- **Endpoint URL**: `https://gcsim-mcp.kgtkr.net/mcp`
- **Transport**: Streamable HTTP (SSE)

### MCP Client Configuration

#### Claude Desktop (`claude_desktop_config.json`)

```json
{
  "mcpServers": {
    "gcsim": {
      "url": "https://gcsim-mcp.kgtkr.net/mcp"
    }
  }
}
```

> [!WARNING]
> **Resource Notice / Disclaimer**
> This public endpoint is hosted on a small demonstration server with strictly limited computing resources. If traffic spikes or concurrent simulations exceed capacity, requests may be throttled or rejected with a `Server busy: Maximum concurrent simulations reached` error. For stable, uninterrupted, or heavy usage, please self-host your own instance.

## Available Tools

- `validate_gcsim_config`: Validates a Genshin Impact combat simulation configuration (GCSL) script and checks syntax and parsed team structure.
- `run_gcsim_sample`: Executes a single simulation sample and returns detailed summary metrics including total DPS, character damage breakdown, action counts, and downtime due to cooldowns/energy.
- `get_gcsim_reference`: Retrieves valid identifier keys and syntax rules for characters, weapons, or artifact sets.
- `get_gcsim_help`: Returns general GCSL syntax help, instructions, and templates.

## Self-Hosting

### Using Docker

```bash
docker run -p 3485:3485 ghcr.io/kgtkr/gcsim-mcp:latest
```

### Configuration Options

The server can be configured via environment variables or CLI flags:

| Environment Variable  | CLI Flag                | Default   | Description                              |
| :-------------------- | :---------------------- | :-------- | :--------------------------------------- |
| `GCSIM_HOST`          | `--host`                | `0.0.0.0` | Listen host for HTTP server              |
| `GCSIM_PORT`          | `--port`                | `3485`    | Listen port for HTTP server              |
| `MAX_CONCURRENT_SIMS` | `--max-concurrent-sims` | `4`       | Maximum concurrent simulations allowed   |
| `MAX_FUEL_VALIDATE`   | `--max-fuel-validate`   | `50M`     | Maximum instruction fuel for validation  |
| `MAX_FUEL_SAMPLE`     | `--max-fuel-sample`     | `500M`    | Maximum instruction fuel for simulation  |
| `MAX_MEMORY`          | `--max-memory`          | `256Mi`   | Maximum Wasm linear memory limit         |
| `AUTH_TOKEN`          | `--auth-token`          | `None`    | Optional Bearer token for authentication |

### Running in stdio mode (Local)

```bash
gcsim-mcp --stdio
```
