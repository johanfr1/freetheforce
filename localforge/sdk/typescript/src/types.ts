/**
 * Type definitions for LocalForge SDK
 */

/** Identity information */
export interface Identity {
    publicKey: string;
    createdAt: string;
    alias?: string;
}

/** Signed entitlement grant */
export interface Grant {
    id: string;
    feature: string;
    subject: string;
    issuer: string;
    validFrom: string;
    validUntil: string;
    signature: string;
}

/** Daemon status */
export interface DaemonStatus {
    version: string;
    status: string;
    uptimeSeconds: number;
    identity?: string;
    grantsActive: number;
    grantsExpired: number;
}

/** Entitlement check result */
export interface CanResult {
    allowed: boolean;
    reason: string;
}

/** Connection options */
export interface ConnectOptions {
    /** Unix socket path (Linux) or endpoint.json path (Windows) */
    socketPath?: string;
    /** Connection timeout in milliseconds */
    timeout?: number;
}

/** JSON-RPC error */
export interface RpcError {
    code: number;
    message: string;
    data?: unknown;
}

/** Forge client interface */
export interface ForgeClient {
    /** Identity operations */
    identity: {
        /** Get current identity */
        get(): Promise<Identity>;
        /** Initialize identity (creates keypair) */
        init(): Promise<Identity>;
        /** Sign a payload (base64 in, base64 out) */
        sign(payloadBase64: string): Promise<string>;
        /** Set identity alias */
        setAlias(alias: string): Promise<void>;
        /** Export identity bundle */
        export(): Promise<string>;
        /** Import identity bundle */
        import(bundle: string): Promise<Identity>;
    };

    /** Entitlements operations */
    entitlements: {
        /** Check if feature is allowed */
        can(feature: string): Promise<CanResult>;
        /** List all grants */
        list(): Promise<Grant[]>;
        /** Add a grant */
        add(grant: Grant): Promise<string>;
        /** Remove a grant by ID */
        remove(id: string): Promise<void>;
    };

    /** Config operations */
    config: {
        /** Get a config value */
        get<T = unknown>(namespace: string, key: string): Promise<T | null>;
        /** Set a config value */
        set<T = unknown>(namespace: string, key: string, value: T): Promise<void>;
        /** List all config values in a namespace */
        list(namespace: string): Promise<Record<string, unknown>>;
        /** Reset namespace to defaults */
        reset(namespace: string): Promise<void>;
    };

    /** Get daemon status */
    status(): Promise<DaemonStatus>;

    /** Close the connection */
    close(): void;
}
