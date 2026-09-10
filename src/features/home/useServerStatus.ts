import { useEffect, useState } from "react";

import { ipc } from "../../lib/ipc";
import type { ServerStatus } from "../../lib/types";

/** Polls the status of `host:port` every `intervalSeconds`. */
export function useServerStatus(host: string | null, port: number | null, intervalSeconds: number) {
  const [status, setStatus] = useState<ServerStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!host) {
      setStatus(null);
      return;
    }
    let cancelled = false;
    let timer: number | undefined;
    const run = async () => {
      setLoading(true);
      try {
        const result = await ipc.serverStatus(host, port);
        if (!cancelled) setStatus(result);
      } catch {
        if (!cancelled) setStatus(null);
      } finally {
        if (!cancelled) setLoading(false);
      }
      if (!cancelled) timer = window.setTimeout(run, Math.max(10, intervalSeconds) * 1000);
    };
    void run();
    return () => {
      cancelled = true;
      if (timer) window.clearTimeout(timer);
    };
  }, [host, port, intervalSeconds, tick]);

  return { status, loading, refresh: () => setTick((n) => n + 1) };
}
