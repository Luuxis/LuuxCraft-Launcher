/**
 * Frontend logging forwarded to the Rust log file through `tauri-plugin-log`.
 * Never pass tokens, passwords or codes to these functions.
 */
import { debug, error, info, warn } from "@tauri-apps/plugin-log";

function safe(promise: Promise<void>) {
  promise.catch(() => {
    /* logging must never break the UI */
  });
}

export const logger = {
  debug: (message: string) => safe(debug(`[ui] ${message}`)),
  info: (message: string) => safe(info(`[ui] ${message}`)),
  warn: (message: string) => safe(warn(`[ui] ${message}`)),
  error: (message: string) => safe(error(`[ui] ${message}`)),
};
