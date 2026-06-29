export async function abortableLoad<T>(
  signal: AbortSignal,
  fetcher: () => Promise<T>,
  fallback: T,
): Promise<T> {
  try {
    const result = await fetcher();
    if (signal.aborted) return fallback;
    return result;
  } catch (e) {
    return fallback;
  }
}
