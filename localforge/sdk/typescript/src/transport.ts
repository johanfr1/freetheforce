/**
 * Transport layer for connecting to the LocalForge daemon
 *
 * Linux: Unix domain socket
 * Windows: TCP via endpoint.json discovery
 */

import * as net from "node:net";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";
import * as readline from "node:readline";

/** Default socket path by platform */
function getDefaultSocketPath(): string {
    if (process.platform === "win32") {
        const localAppData = process.env.LOCALAPPDATA || path.join(os.homedir(), "AppData", "Local");
        return path.join(localAppData, "LocalForge", "endpoint.json");
    }
    return path.join(os.homedir(), ".localforge", "forge.sock");
}

/** TCP endpoint info (Windows) */
interface TcpEndpoint {
    type: string;
    host: string;
    port: number;
}

/** Read endpoint.json for Windows TCP connection */
function readEndpoint(endpointPath: string): TcpEndpoint {
    const content = fs.readFileSync(endpointPath, "utf-8");
    return JSON.parse(content) as TcpEndpoint;
}

/** Transport connection */
export class Transport {
    private socket: net.Socket | null = null;
    private rl: readline.Interface | null = null;
    private responseQueue: Array<(line: string) => void> = [];

    /** Connect to daemon */
    async connect(socketPath?: string): Promise<void> {
        const targetPath = socketPath || getDefaultSocketPath();

        return new Promise((resolve, reject) => {
            if (process.platform === "win32") {
                // Windows: TCP via endpoint.json
                if (!fs.existsSync(targetPath)) {
                    reject(new Error("Daemon not running. Start with 'forge-daemon'"));
                    return;
                }

                const endpoint = readEndpoint(targetPath);
                this.socket = net.createConnection({ host: endpoint.host, port: endpoint.port }, () => {
                    this.setupLineReader();
                    resolve();
                });
            } else {
                // Unix: domain socket
                if (!fs.existsSync(targetPath)) {
                    reject(new Error("Daemon not running. Start with 'forge-daemon'"));
                    return;
                }

                this.socket = net.createConnection({ path: targetPath }, () => {
                    this.setupLineReader();
                    resolve();
                });
            }

            this.socket.on("error", (err) => {
                reject(new Error(`Connection failed: ${err.message}`));
            });
        });
    }

    /** Set up line-based reading for JSON-RPC responses */
    private setupLineReader(): void {
        if (!this.socket) return;

        this.rl = readline.createInterface({
            input: this.socket,
            crlfDelay: Infinity,
        });

        this.rl.on("line", (line) => {
            const handler = this.responseQueue.shift();
            if (handler) {
                handler(line);
            }
        });
    }

    /** Send a line and wait for response */
    async send(line: string): Promise<string> {
        if (!this.socket) {
            throw new Error("Not connected");
        }

        return new Promise((resolve, reject) => {
            this.responseQueue.push(resolve);

            this.socket!.write(line + "\n", (err) => {
                if (err) {
                    this.responseQueue.pop();
                    reject(err);
                }
            });
        });
    }

    /** Close the connection */
    close(): void {
        if (this.rl) {
            this.rl.close();
            this.rl = null;
        }
        if (this.socket) {
            this.socket.destroy();
            this.socket = null;
        }
        this.responseQueue = [];
    }

    /** Check if connected */
    get connected(): boolean {
        return this.socket !== null && !this.socket.destroyed;
    }
}
