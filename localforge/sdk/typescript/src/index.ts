/**
 * @localforge/sdk
 *
 * TypeScript SDK for LocalForge daemon
 */

export { connect } from "./client.js";
export type {
    ForgeClient,
    Identity,
    Grant,
    DaemonStatus,
    CanResult,
    ConnectOptions,
    RpcError,
} from "./types.js";
