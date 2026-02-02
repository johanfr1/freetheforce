# @localforge/sdk

TypeScript SDK for the LocalForge daemon.

## Installation

```bash
npm install @localforge/sdk
```

## Usage

```typescript
import { connect } from "@localforge/sdk";

// Connect to daemon
const client = await connect();

// Get identity
const identity = await client.identity.get();
console.log(`Public key: ${identity.publicKey}`);

// Check entitlement
const { allowed, reason } = await client.entitlements.can("pro.export");
console.log(`Allowed: ${allowed} (${reason})`);

// Get/set config
const endpoint = await client.config.get("myapp", "api_endpoint");
await client.config.set("myapp", "timeout", 30);

// Get daemon status
const status = await client.status();
console.log(`Daemon v${status.version}, uptime: ${status.uptimeSeconds}s`);

// Cleanup
client.close();
```

## API

### `connect(options?): Promise<ForgeClient>`

Connect to the LocalForge daemon.

- **options.socketPath**: Custom socket path (Linux) or endpoint.json path (Windows)
- **options.timeout**: Connection timeout in milliseconds

### `ForgeClient.identity`

- `get()`: Get current identity
- `init()`: Initialize identity (creates keypair)
- `sign(payloadBase64)`: Sign a payload
- `setAlias(alias)`: Set identity alias
- `export()`: Export identity bundle
- `import(bundle)`: Import identity bundle

### `ForgeClient.entitlements`

- `can(feature)`: Check if feature is allowed
- `list()`: List all grants
- `add(grant)`: Add a grant
- `remove(id)`: Remove a grant by ID

### `ForgeClient.config`

- `get(namespace, key)`: Get a config value
- `set(namespace, key, value)`: Set a config value
- `list(namespace)`: List all config values
- `reset(namespace)`: Reset namespace to defaults

### `ForgeClient.status()`

Get daemon status.

### `ForgeClient.close()`

Close the connection.

## Platform Support

- **Linux**: Unix domain socket at `~/.localforge/forge.sock`
- **Windows**: TCP via `%LOCALAPPDATA%\LocalForge\endpoint.json`

## License

MIT
