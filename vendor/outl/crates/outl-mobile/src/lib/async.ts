/**
 * Async helpers shared across the mobile client.
 *
 * Right now this is a single primitive — `withTimeout` — but it
 * lives alone in its own module so the callers don't have to import
 * from an unrelated grab-bag. Add to it sparingly; helpers that fit
 * a specific domain (e.g. textarea, outline) should go there
 * instead.
 */

/**
 * Race a promise against a timeout. If `op` doesn't settle within
 * `ms` milliseconds, reject with `Error(label + " (timed out)")`.
 *
 * Used to keep Tauri commands from hanging the UI forever when the
 * native side stalls (iCloud coordination, ops/ download pending,
 * background app refresh racing the foreground request). Callers
 * should surface the rejection via the same error path they already
 * have — the UI stays consistent whether the failure was an error
 * or a timeout.
 */
export function withTimeout<T>(
  op: Promise<T>,
  ms: number,
  label: string,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`${label} (timed out after ${ms}ms)`)),
      ms,
    );
  });
  return Promise.race([op, timeout]).finally(() => {
    if (timer !== undefined) clearTimeout(timer);
  }) as Promise<T>;
}
