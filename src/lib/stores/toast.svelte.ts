export type ToastLevel = "error" | "warning" | "info" | "success";
export type ToastSource = "api" | "sync" | "player" | "system";

export interface ToastAction {
  label: string;
  onClick: () => void | Promise<void>;
}

export interface ToastItem {
  id: string;
  level: ToastLevel;
  source: ToastSource;
  title: string;
  message: string;
  dismissAfterMs: number;
  createdAt: number;
  action?: ToastAction;
}

export interface PushToastInput {
  level: ToastLevel;
  source: ToastSource;
  title?: string;
  message: string;
  dismissAfterMs?: number;
  dedupeKey?: string;
  action?: ToastAction;
}

const MAX_TOASTS = 5;
const DEFAULT_DISMISS_MS = 6000;
const DEDUPE_WINDOW_MS = 5000;

let queue = $state<ToastItem[]>([]);
let nextId = 1;
const recentByKey = new Map<string, number>();

function now(): number {
  return Date.now();
}

function compactRecentMap(timestamp: number): void {
  for (const [key, seenAt] of recentByKey.entries()) {
    if (timestamp - seenAt > DEDUPE_WINDOW_MS) {
      recentByKey.delete(key);
    }
  }
}

function defaultTitle(level: ToastLevel, source: ToastSource): string {
  if (source === "sync")
    return level === "error" ? "Sync failed" : "Sync update";
  if (source === "player")
    return level === "error" ? "Playback error" : "Playback update";
  if (source === "api")
    return level === "error" ? "Request failed" : "Request update";
  return level === "error" ? "Error" : "Notification";
}

export function getToasts(): ToastItem[] {
  return queue;
}

export function dismissToast(id: string): void {
  queue = queue.filter((toast) => toast.id !== id);
}

export function clearToasts(): void {
  queue = [];
}

export function pushToast(input: PushToastInput): string | null {
  const message = input.message?.trim();
  if (!message) return null;

  const timestamp = now();
  compactRecentMap(timestamp);

  const dedupeKey =
    input.dedupeKey ??
    `${input.source}|${input.level}|${input.title ?? ""}|${message}`;
  const previouslySeenAt = recentByKey.get(dedupeKey);

  if (previouslySeenAt && timestamp - previouslySeenAt < DEDUPE_WINDOW_MS) {
    return null;
  }

  recentByKey.set(dedupeKey, timestamp);

  const id = `toast-${nextId++}`;
  const toast: ToastItem = {
    id,
    level: input.level,
    source: input.source,
    title: input.title?.trim() || defaultTitle(input.level, input.source),
    message,
    dismissAfterMs: input.dismissAfterMs === 0 ? 0 : Math.max(1200, input.dismissAfterMs ?? DEFAULT_DISMISS_MS),
    createdAt: timestamp,
    action: input.action,
  };

  queue = [toast, ...queue].slice(0, MAX_TOASTS);

  if (toast.dismissAfterMs > 0) {
    setTimeout(() => {
      dismissToast(id);
    }, toast.dismissAfterMs);
  }

  return id;
}

export function normalizeErrorMessage(error: unknown): string {
  if (!error) return "Unknown error";

  if (typeof error === "string") {
    return error;
  }

  if (typeof error === "object") {
    const maybeRecord = error as Record<string, unknown>;

    if (typeof maybeRecord.message === "string" && maybeRecord.message.trim()) {
      return maybeRecord.message;
    }

    if (
      typeof maybeRecord.kind === "string" &&
      typeof maybeRecord.message === "string"
    ) {
      return `${maybeRecord.kind}: ${maybeRecord.message}`;
    }

    if (typeof maybeRecord.error === "string") {
      return maybeRecord.error;
    }

    if (
      maybeRecord.error !== undefined &&
      maybeRecord.error !== null &&
      typeof maybeRecord.error === "object"
    ) {
      return normalizeErrorMessage(maybeRecord.error);
    }

    if (typeof maybeRecord.reason === "string" && maybeRecord.reason.trim()) {
      return maybeRecord.reason;
    }

    if (typeof maybeRecord.detail === "string" && maybeRecord.detail.trim()) {
      return maybeRecord.detail;
    }

    if (typeof maybeRecord.err === "string" && maybeRecord.err.trim()) {
      return maybeRecord.err;
    }

    if (
      typeof (error as any).toString === "function" &&
      (error as any).toString !== Object.prototype.toString
    ) {
      return (error as any).toString();
    }

    try {
      return JSON.stringify(error);
    } catch {
      // Fall through
    }
  }

  return String(error);
}


export function pushErrorToast(
  source: ToastSource,
  error: unknown,
  title?: string,
  dedupeKey?: string,
): string | null {
  return pushToast({
    level: "error",
    source,
    title,
    message: normalizeErrorMessage(error),
    dedupeKey,
  });
}

export function updateToast(
  id: string,
  updates: Partial<Pick<ToastItem, "message" | "title" | "action" | "level" | "dismissAfterMs">>
): void {
  queue = queue.map((toast) => {
    if (toast.id === id) {
      return { ...toast, ...updates };
    }
    return toast;
  });
}
