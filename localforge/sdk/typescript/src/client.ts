/**
 * LocalForge SDK Client
 *
 * Provides a typed interface to the LocalForge daemon.
 */

import { Transport } from "./transport.js";
import { RpcClient } from "./rpc.js";
import type {
    ForgeClient,
    Identity,
    Grant,
    DaemonStatus,
    CanResult,
    ConnectOptions,
} from "./types.js";

/** Create the identity API */
function createIdentityApi(rpc: RpcClient): ForgeClient["identity"] {
    return {
        async get(): Promise<Identity> {
            return rpc.call<Identity>("identity.get");
        },

        async init(): Promise<Identity> {
            return rpc.call<Identity>("identity.init");
        },

        async sign(payloadBase64: string): Promise<string> {
            const result = await rpc.call<{ signature: string }>("identity.sign", {
                payload: payloadBase64,
            });
            return result.signature;
        },

        async setAlias(alias: string): Promise<void> {
            await rpc.call("identity.setAlias", { alias });
        },

        async export(): Promise<string> {
            const result = await rpc.call<{ bundle: string }>("identity.export");
            return result.bundle;
        },

        async import(bundle: string): Promise<Identity> {
            return rpc.call<Identity>("identity.import", { bundle });
        },
    };
}

/** Create the entitlements API */
function createEntitlementsApi(rpc: RpcClient): ForgeClient["entitlements"] {
    return {
        async can(feature: string): Promise<CanResult> {
            return rpc.call<CanResult>("entitlements.can", { feature });
        },

        async list(): Promise<Grant[]> {
            const result = await rpc.call<{ grants: Grant[] }>("entitlements.list");
            return result.grants;
        },

        async add(grant: Grant): Promise<string> {
            const result = await rpc.call<{ id: string }>("entitlements.add", { grant });
            return result.id;
        },

        async remove(id: string): Promise<void> {
            await rpc.call("entitlements.remove", { id });
        },
    };
}

/** Create the config API */
function createConfigApi(rpc: RpcClient): ForgeClient["config"] {
    return {
        async get<T = unknown>(namespace: string, key: string): Promise<T | null> {
            const result = await rpc.call<{ value: T | null }>("config.get", { namespace, key });
            return result.value;
        },

        async set<T = unknown>(namespace: string, key: string, value: T): Promise<void> {
            await rpc.call("config.set", { namespace, key, value });
        },

        async list(namespace: string): Promise<Record<string, unknown>> {
            const result = await rpc.call<{ entries: Record<string, unknown> }>("config.list", {
                namespace,
            });
            return result.entries;
        },

        async reset(namespace: string): Promise<void> {
            await rpc.call("config.reset", { namespace });
        },
    };
}

/** Internal client implementation */
class ForgeClientImpl implements ForgeClient {
    private transport: Transport;
    private rpc: RpcClient;

    identity: ForgeClient["identity"];
    entitlements: ForgeClient["entitlements"];
    config: ForgeClient["config"];

    constructor(transport: Transport) {
        this.transport = transport;
        this.rpc = new RpcClient(transport);

        this.identity = createIdentityApi(this.rpc);
        this.entitlements = createEntitlementsApi(this.rpc);
        this.config = createConfigApi(this.rpc);
    }

    async status(): Promise<DaemonStatus> {
        return this.rpc.call<DaemonStatus>("status");
    }

    close(): void {
        this.transport.close();
    }
}

/**
 * Connect to the LocalForge daemon
 *
 * @param options Connection options
 * @returns Connected client instance
 *
 * @example
 * ```typescript
 * import { connect } from "@localforge/sdk";
 *
 * const client = await connect();
 *
 * // Get identity
 * const identity = await client.identity.get();
 * console.log(identity.publicKey);
 *
 * // Check entitlement
 * const { allowed } = await client.entitlements.can("pro.export");
 *
 * // Get config
 * const endpoint = await client.config.get("myapp", "api_endpoint");
 *
 * // Cleanup
 * client.close();
 * ```
 */
export async function connect(options: ConnectOptions = {}): Promise<ForgeClient> {
    const transport = new Transport();
    await transport.connect(options.socketPath);
    return new ForgeClientImpl(transport);
}
