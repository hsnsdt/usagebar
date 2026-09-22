import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Snapshot } from "../types";

/**
 * Subscribes to `usage-updated` pushed from Rust. Never fetches anything
 * itself. Also exposes a 1 Hz clock for live countdowns.
 */
export function useUsage(): { snapshot: Snapshot | null; now: Date } {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    let alive = true;
    invoke<Snapshot>("get_snapshot")
      .then((s) => alive && setSnapshot(s))
      .catch(() => {});
    const un = listen<Snapshot>("usage-updated", (e) => {
      if (alive) setSnapshot(e.payload);
    });
    return () => {
      alive = false;
      un.then((f) => f()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(id);
  }, []);

  return { snapshot, now };
}

export function refreshNow(): Promise<void> {
  return invoke("refresh_now");
}

export function hidePopup(): Promise<void> {
  return invoke("hide_popup");
}
