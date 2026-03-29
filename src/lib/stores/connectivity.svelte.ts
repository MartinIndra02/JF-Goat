let online = $state(
  typeof navigator === "undefined" ? true : navigator.onLine,
);
let degraded = $state(false);
let lastNetworkError = $state<string | null>(null);
let lastSuccessfulSyncAt = $state<number | null>(null);

export function isOnline(): boolean {
  return online;
}

export function isDegraded(): boolean {
  return degraded;
}

export function getLastNetworkError(): string | null {
  return lastNetworkError;
}

export function getLastSuccessfulSyncAt(): number | null {
  return lastSuccessfulSyncAt;
}

export function setOnlineStatus(next: boolean): void {
  online = next;
  if (!next) {
    degraded = true;
  }
}

export function markDegraded(message: string): void {
  degraded = true;
  lastNetworkError = message;
}

export function markHealthy(): void {
  degraded = false;
  lastNetworkError = null;
  lastSuccessfulSyncAt = Date.now();
}
