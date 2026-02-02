# LocalForge

A local-first developer substrate providing shared infrastructure for identity, trust, entitlements, config, and logging.

## Philosophy

- **Offline-first**: All operations work without network
- **Inspectable**: All state is human-readable files in `~/.localforge/`
- **Minimal**: Single daemon, single socket, predictable API
- **Calm**: Explicit operations, descriptive errors, no surprises

## Quick Start

```bash
# Build
cargo build --release

# Start daemon (in one terminal)
./target/release/forge-daemon

# Initialize identity (in another terminal)
./target/release/forge init

# Check status
./target/release/forge status
```

## Data Directory

All state is stored in:
- **Linux**: `~/.localforge/`
- **Windows**: `%LOCALAPPDATA%\LocalForge\`

```
<DATA_DIR>/
├── identity/           # Ed25519 keypair
├── entitlements/       # Signed grants
├── config/             # Per-project configs
└── logs/               # Daily structured logs
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `forge init` | Initialize identity |
| `forge identity` | Show identity info |
| `forge can <feature>` | Check entitlement |
| `forge config get/set` | Manage config |
| `forge status` | Daemon status |
| `forge logs` | View logs |

## API

JSON-RPC 2.0 over Unix domain socket (Linux) or TCP (Windows).

See `docs/api-reference.md` for full API documentation.

## License

MIT
