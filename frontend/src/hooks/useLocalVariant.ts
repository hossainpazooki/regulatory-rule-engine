/**
 * Flag-select helper for Gate-5 local-surface rewiring (spec § 7.4).
 *
 * `selectQueryFn` picks the local variant when its flag is on, else the canonical
 * `VITE_API_URL` fallback. When the local variant throws `ServeUnsupportedError`
 * (a SCAFFOLD-ONLY page with no local surface yet), it transparently retries with
 * the fallback so a flag-on scaffold page behaves EXACTLY as today.
 *
 * With the flag off, the returned function IS the fallback unchanged - so with all
 * flags off the rewire is a no-op and `main` is byte-unchanged in behavior.
 */
import { ServeUnsupportedError } from '@/api/serve/serveClient'

export function selectQueryFn<T>(
  flagOn: boolean,
  localFn: () => Promise<T>,
  fallbackFn: () => Promise<T>,
): () => Promise<T> {
  if (!flagOn) return fallbackFn
  return async () => {
    try {
      return await localFn()
    } catch (err) {
      // SCAFFOLD-ONLY pages signal "no local surface yet" with this typed error;
      // fall back to the canonical path so behavior is identical to today. Any
      // OTHER error is a genuine local-surface failure and must surface.
      if (err instanceof ServeUnsupportedError) {
        return fallbackFn()
      }
      throw err
    }
  }
}
