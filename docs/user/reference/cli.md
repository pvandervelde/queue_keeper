# CLI Reference

The `queue-keeper` binary provides a command-line interface for managing the service.

**Global flags** (available on all subcommands):

| Flag | Env var | Default | Description |
|---|---|---|---|
| `-c`, `--config <PATH>` | `QUEUE_KEEPER_CONFIG` | — | Path to `service.yaml` |
| `-l`, `--log-level <LEVEL>` | — | `info` | Log level: `trace`, `debug`, `info`, `warn`, `error` |
| `--json-logs` | — | off | Emit JSON-formatted log lines |

---

## `queue-keeper start`

Start the Queue-Keeper service.

```
queue-keeper start [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-m`, `--mode <MODE>` | `server` | Service mode: `server`, `worker`, `combined` |
| `-p`, `--port <PORT>` | `8080` | HTTP server port |
| `--host <HOST>` | `0.0.0.0` | Interface to bind |
| `-f`, `--foreground` | off | Run in the foreground (do not daemonise) |

**Modes:**

| Mode | Description |
|---|---|
| `server` | Accept and process incoming webhooks |
| `worker` | Process queued events only (no HTTP listener) |
| `combined` | Both server and worker in one process |

**Example:**

```bash
queue-keeper start --foreground --mode combined --port 8080
```

---

## `queue-keeper stop`

Gracefully stop the running service.

```
queue-keeper stop [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-t`, `--timeout <SECS>` | `30` | Wait up to this many seconds for graceful shutdown |
| `-f`, `--force` | off | Force-kill if graceful shutdown times out |

---

## `queue-keeper status`

Show the current service status.

```
queue-keeper status [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-v`, `--verbose` | off | Show component-level detail |
| `-o`, `--format <FORMAT>` | `text` | Output format: `text`, `json`, `yaml`, `table` |

---

## `queue-keeper config`

Validate and inspect configuration.

```
queue-keeper config [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-f`, `--file <PATH>` | from `--config` | Configuration file to validate |
| `-s`, `--show` | off | Print the resolved configuration |
| `--format <FORMAT>` | `yaml` | Output format when `--show`: `yaml`, `json`, `toml` |

**Example — validate and show:**

```bash
queue-keeper config --file /etc/queue-keeper/bot-config.yaml --show
```

---

## `queue-keeper monitor`

Stream live event processing activity.

```
queue-keeper monitor [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-f`, `--follow` | off | Continuously stream new events |
| `-e`, `--event-type <TYPE>` | — | Filter by event type |
| `-r`, `--repository <REPO>` | — | Filter by `owner/repo` |
| `--errors-only` | off | Show only failed events |
| `-n`, `--limit <N>` | `100` | Number of recent events to show initially |

---

## `queue-keeper events`

Sub-commands for managing processed events.

### `queue-keeper events list`

```
queue-keeper events list [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-n`, `--limit <N>` | `50` | Max events to list |
| `-e`, `--event-type <TYPE>` | — | Filter by event type |
| `-r`, `--repository <REPO>` | — | Filter by `owner/repo` |
| `-s`, `--session <ID>` | — | Filter by session ID |
| `-S`, `--since <TIMESTAMP>` | — | Events after this ISO 8601 timestamp |
| `-o`, `--format <FORMAT>` | `table` | Output format |

### `queue-keeper events show <EVENT_ID>`

Show full details for one event.

| Flag | Default | Description |
|---|---|---|
| `-o`, `--format <FORMAT>` | `yaml` | Output format |
| `--raw` | off | Show original webhook payload |

### `queue-keeper events replay <EVENT_ID>`

Replay an event from Blob Storage.

| Flag | Default | Description |
|---|---|---|
| `-f`, `--force` | off | Replay even if already processed |
| `-q`, `--queue <NAME>` | all matching | Route only to this queue |

### `queue-keeper events delete <EVENT_ID>`

Delete an event record.

| Flag | Default | Description |
|---|---|---|
| `-y`, `--yes` | off | Skip confirmation prompt |

---

## `queue-keeper sessions`

Sub-commands for managing processing sessions.

### `queue-keeper sessions list`

```
queue-keeper sessions list [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-r`, `--repository <REPO>` | — | Filter by repository |
| `-e`, `--entity-type <TYPE>` | — | Filter by entity type |
| `-p`, `--pending-only` | off | Show only sessions with pending events |
| `-o`, `--format <FORMAT>` | `table` | Output format |

### `queue-keeper sessions show <SESSION_ID>`

Show details for a session.

| Flag | Default | Description |
|---|---|---|
| `-o`, `--format <FORMAT>` | `yaml` | Output format |
| `--with-events` | off | Include full event history |

### `queue-keeper sessions reset <SESSION_ID>`

Reset session state.

| Flag | Default | Description |
|---|---|---|
| `-y`, `--yes` | off | Skip confirmation |
| `-r`, `--reason <TEXT>` | — | Reason for reset |

### `queue-keeper sessions pause <SESSION_ID>`

Pause processing for a session.

| Flag | Default | Description |
|---|---|---|
| `-r`, `--reason <TEXT>` | — | Reason for pause |

### `queue-keeper sessions resume <SESSION_ID>`

Resume a paused session.

---

## `queue-keeper health`

Sub-commands for health checks.

### `queue-keeper health check`

```
queue-keeper health check [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-v`, `--verbose` | off | Include per-component detail |
| `-t`, `--timeout <SECS>` | `10` | Timeout for each check |
| `-o`, `--format <FORMAT>` | `text` | Output format |

### `queue-keeper health queue`

```
queue-keeper health queue [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-p`, `--provider <NAME>` | all | Queue provider to check |
| `-s`, `--stats` | off | Include queue statistics |

### `queue-keeper health github`

```
queue-keeper health github [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `-a`, `--auth` | off | Test authentication |
| `--rate-limits` | off | Check API rate limit status |

### `queue-keeper health storage`

```
queue-keeper health storage [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `--storage-type <TYPE>` | all | Storage type to check |
| `-s`, `--stats` | off | Include storage statistics |

---

## `queue-keeper completions <SHELL>`

Generate shell completion scripts.

```bash
# Bash
queue-keeper completions bash > /etc/bash_completion.d/queue-keeper

# Zsh
queue-keeper completions zsh > ~/.zsh/completions/_queue-keeper

# Fish
queue-keeper completions fish > ~/.config/fish/completions/queue-keeper.fish

# PowerShell
queue-keeper completions powershell | Out-String | Invoke-Expression
```

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Configuration error |
| `2` | Service error |
| `3` | Command failed |
| `4` | Invalid argument |
| `5` | I/O error |
| `6` | Queue-Keeper internal error |
