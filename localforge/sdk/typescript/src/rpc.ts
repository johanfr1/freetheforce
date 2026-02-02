/**
 * JSON-RPC 2.0 client for LocalForge daemon
 */

import { Transport } from "./transport.js";
import type { RpcError } from "./types.js";

/** JSON-RPC request */
interface JsonRpcRequest {
    jsonrpc: "2.0";
    id: number;
    method: string;
    params: unknown;
}

/** JSON-RPC response */
interface JsonRpcResponse {
    jsonrpc: string;
    id: number | null;
    result?: unknown;
    error?: RpcError;
}

/** RPC client */
export class RpcClient {
    private transport: Transport;
    private nextId = 1;

    constructor(transport: Transport) {
        this.transport = transport;
    }

    /** Call an RPC method */
    async call<T>(method: string, params: unknown = {}): Promise<T> {
        const request: JsonRpcRequest = {
            jsonrpc: "2.0",
            id: this.nextId++,
            method,
            params,
        };

        const requestJson = JSON.stringify(request);
        const responseJson = await this.transport.send(requestJson);

        let response: JsonRpcResponse;
        try {
            response = JSON.parse(responseJson);
        } catch {
            throw new Error("Invalid JSON response from daemon");
        }

        if (response.error) {
            const err = new Error(response.error.message) as Error & { code: number; data?: unknown };
            err.code = response.error.code;
            err.data = response.error.data;
            throw err;
        }

        return response.result as T;
    }
}
