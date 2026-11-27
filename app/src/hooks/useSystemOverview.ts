import { useEffect, useState } from 'react';
import { getSystemOverview, type SystemOverview } from '../api';
import {
  OVERVIEW_POLL_MS,
  memoryPercent,
  pushHistorySample,
} from '../overviewModel';

export interface SystemOverviewState {
  data: SystemOverview | null;
  cpuHistory: number[];
  memHistory: number[];
  error: string | null;
}

/**
 * Polls `get_system_overview` every 2 seconds ONLY while mounted and while
 * `document.visibilityState === 'visible'`. Maintains a 30-point (~60s) ring buffer
 * for CPU and memory sparklines and guarantees zero background IPC when unmounted.
 */
export function useSystemOverview(): SystemOverviewState {
  const [state, setState] = useState<SystemOverviewState>({
    data: null,
    cpuHistory: [],
    memHistory: [],
    error: null,
  });

  useEffect(() => {
    let alive = true;
    let inFlight = false;
    let timer: number | null = null;

    const sample = async () => {
      if (!alive || inFlight) return;
      if (typeof document !== 'undefined' && document.visibilityState !== 'visible') return;
      inFlight = true;
      try {
        const snapshot = await getSystemOverview();
        if (!alive) return;
        const memPct = memoryPercent(snapshot.mem_used_bytes, snapshot.mem_total_bytes);
        setState((prev) => ({
          data: snapshot,
          cpuHistory: pushHistorySample(prev.cpuHistory, snapshot.cpu_usage_pct),
          memHistory: pushHistorySample(prev.memHistory, memPct),
          error: null,
        }));
      } catch (err) {
        if (!alive) return;
        setState((prev) => ({ ...prev, error: String(err) }));
      } finally {
        inFlight = false;
      }
    };

    const stopPolling = () => {
      if (timer !== null) {
        window.clearInterval(timer);
        timer = null;
      }
    };

    const startPolling = () => {
      if (!alive || timer !== null) return;
      void sample();
      timer = window.setInterval(() => {
        void sample();
      }, OVERVIEW_POLL_MS);
    };

    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        startPolling();
      } else {
        stopPolling();
      }
    };

    if (typeof document === 'undefined' || document.visibilityState === 'visible') {
      startPolling();
    }
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', onVisibilityChange);
    }

    return () => {
      alive = false;
      stopPolling();
      if (typeof document !== 'undefined') {
        document.removeEventListener('visibilitychange', onVisibilityChange);
      }
    };
  }, []);

  return state;
}
