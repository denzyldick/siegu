/**
 * Transport-casing helpers shared by every browser data plane (webHost HTTP and
 * guest WebRTC). They translate the camelCase payload keys the UI/tests pass in
 * to the snake_case keys Rust `dispatch` reads (`crates/siegu-core/src/rpc.rs`),
 * using the generated catalog (`shared/generated/rpc-commands.ts`) as the single
 * source of truth for command names and their arg keys.
 *
 * The Tauri IPC bridge does this conversion automatically; the browser transports
 * do not, so both webHost and guest funnel through `toSnakeCaseKeys`.
 */
import { RPC_COMMANDS, type RpcCommandSpec } from 'shared/generated/rpc-commands';

/** Union of every command the Rust host catalog knows (typed seam). */
export type CommandName = RpcCommandSpec['name'];

/**
 * Accept any string but autocompletes/validates known catalog commands. Lets the
 * typed seam grow without breaking callers that pass non-catalog strings today.
 */
export type CommandOrString = CommandName | (string & {});

/** All command names known to the Rust host (indexed set). */
export const RPC_NAMES: ReadonlySet<string> = new Set(RPC_COMMANDS.map((c) => c.name));

/** catalog command name -> its snake_case arg keys (source of truth). */
export const RPC_ARGS_BY_NAME: ReadonlyMap<string, readonly string[]> = new Map(
  RPC_COMMANDS.map((c) => [c.name, c.args]),
);

/** Is `name` a command the Rust host recognizes? */
export function knownCommand(name: string): boolean {
  return RPC_NAMES.has(name);
}

/** The snake_case arg keys Rust expects for `name` (empty for unknown). */
export function rpcArgs(name: string): readonly string[] {
  return RPC_ARGS_BY_NAME.get(name) ?? [];
}

/** Convert a single camelCase key to snake_case (id-like/snake keys untouched). */
function snakeKey(key: string): string {
  if (key.includes('_')) return key; // already snake_case or self-contained (photo_id/face_id)
  return key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

/**
 * Deeply not required: top-level keys only, matching the RPC dispatcher which
 * reads flat args. camelCase -> snake_case; already-snake and single-word keys
 * are left as-is.
 */
export function toSnakeCaseKeys(payload: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(payload)) {
    out[snakeKey(key)] = value;
  }
  return out;
}
