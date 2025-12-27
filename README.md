# lazyredis

**A simple terminal user interface (TUI) for browsing and managing Redis databases.**

lazyredis allows you to connect to one or more Redis instances, explore keys in a tree view,
inspect and copy values of different types (string, hash, list, set, sorted set, stream), search/filter keys,
delete keys or whole key prefixes, and seed a Redis instance with sample data for testing.

## Key Features

- **Interactive key tree view:** browse keys grouped by delimiter (default `:`) in a folder-like hierarchy.
- **Multi-type value inspector:** view and navigate string, hash, list, set, sorted set (zset), stream, and JSON values.
- **Binary-safe display:** UTF-8 text when possible; hex rendering for non-UTF8 values.
- **Fuzzy search:** quickly find keys across the current view using fuzzy matching.
- **Profile & DB management:** switch between multiple connection profiles and Redis databases (select via `p`).
- **Copy to clipboard:** copy key names or values directly to the system clipboard (`y` / `Y`).
- **Delete keys or prefixes:** delete individual keys or entire key folders (with confirmation, batched UNLINK where available).
- **Pagination & navigation:** navigate values with arrow keys, page up/down, and Tab for focus switching.
- **Seeding test data:** populate a development Redis instance with a large variety of sample keys via `--seed`.

## Installation

### Pre-built / Homebrew (Mac):
```bash
brew tap mazdak/lazyredis
brew install lazyredis
```

Pre-build Linux Binaries are available on Github:
https://github.com/mazdak/lazyredis/releases

### Install From Source:

1. Install [Rust and Cargo](https://rustup.rs/).
2. Clone this repository:
   ```bash
   git clone https://github.com/mazdak/lazyredis.git
   cd lazyredis
   ```
3. Build the release binary:
   ```bash
   cargo build --release
   ```

## Usage

Run the TUI:

```bash
cargo run --release
```

Or use the generated binary:

```bash
./target/release/lazyredis
```

### Command-line Options

```text
lazyredis 0.1.0

USAGE:
    lazyredis [OPTIONS]

OPTIONS:
        --profile <PROFILE>    Specify profile name to connect, or to select for seeding/purging (default: first profile)
        --seed                 Seed the Redis instance with test data (dev only)
        --purge                Purge (delete) all keys in the Redis instance (dev only)
    -h, --help               Print help information
    -V, --version            Print version information
```

When launched normally, lazyredis will:

1. Load connection profiles from `~/.config/lazyredis/lazyredis.toml` (created automatically on first run).
2. Connect to the specified profile (via `--profile`) or to the first profile and database.
3. Enter the TUI for browsing keys/values.

### Basic Controls

| Key/Action          | Description                         |
| ------------------- | ----------------------------------- |
| `q`                 | Quit                                |
| `p`                 | Open profile selector               |
| `j` / `k` / ↓ / ↑   | Navigate keys or values             |
| `Tab` / `Shift+Tab` | Switch focus between panels         |
| `Enter`             | Enter folder / select key           |
| `Esc` / `Backspace` | Go up or exit search/delete mode    |
| `/`                 | Start fuzzy key search              |
| `y`                 | Copy selected key name              |
| `Y`                 | Copy selected key value             |
| `d`                 | Delete selected key or prefix       |
| `PgUp` / `PgDn`     | Page navigation in value view       |

## Configuration

On first run, lazyredis generates a default config file at:

```
~/.config/lazyredis/lazyredis.toml
```

on macOS:
```
~/Library/Application Support/lazyredis/lazyredis.toml
```

Example `lazyredis.toml`:

```toml
[[connections]]
name = "Default"
url = "redis://127.0.0.1:6379"
db = 0
dev = true
color = "lightgreen"
```

- `name`: Human-readable profile name.
- `url`: Redis connection URL.
- `db`: Optional database index (0–15).
- `dev`: Optional flag to mark development profiles (for `--seed` and `--purge`).
- `color`: Optional color for the profile (e.g., in the UI). Accepts common color names (like "red", "green", "lightblue") or hex codes (e.g., "#FF0000"). Defaults to white if not specified or invalid.

To add more profiles, append additional `[[connections]]` tables.
To add more profiles, append additional `[[connections]]` tables. For instance:

```toml
[[connections]]
name = "Local Dev"
url = "redis://127.0.0.1:6379"
db = 0
dev = true
color = "lightgreen" # A nice, light green for local dev

[[connections]]
name = "Staging Server"
url = "redis://staging.example.com:6379"
db = 1
dev = false
color = "#FFA500" # Orange hex code for staging

[[connections]]
name = "Production Read Replica"
url = "redis://prod-replica.example.com:6379"
db = 0
dev = false
color = "red" # Red to indicate caution for production
```


## Seeding and Purging Test Data

You can populate a development Redis instance with sample keys by using:

```bash
lazyredis --seed
# Or specify a profile:
lazyredis --profile Default --seed
```

To purge (delete) all keys in a development Redis instance without seeding, use:

```bash
lazyredis --purge
# Or specify a profile:
lazyredis --profile Default --purge
```

Both commands will only target profiles marked as `dev = true` in your configuration.

This generates (only for `--seed`):

- Simple string keys, nested hierarchies, paths, hashes, lists, sets, sorted sets, streams, JSON-compatible values, and empty types.

## Value Rendering Notes

- **Strings & Binary:** Values are shown as UTF-8 when possible. Non-UTF8 bytes are rendered as hex.
- **JSON:** JSON values (RedisJSON module) are pretty-printed when valid JSON.
- **Streams:** The viewer shows the latest 100 entries using a read-only range query (no consumer groups are created).

## Contributing

Contributions welcome! Please open issues or pull requests on GitHub.
