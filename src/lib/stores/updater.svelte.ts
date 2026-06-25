import { check } from "@tauri-apps/plugin-updater";
import { restartApp } from "../api";

export type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "installing"
  | "upToDate"
  | "readyToRestart"
  | "error";

export type UpdateInfo = {
  version: string;
  date: string;
  body: string;
};

let updateStatus = $state<UpdateStatus>("idle");
let updateInfo = $state<UpdateInfo | null>(null);
let downloadProgress = $state<number | null>(null);
let errorMessage = $state<string>("");

// Keep a reference to the resolved update object so we can install later
let pendingUpdate: Awaited<ReturnType<typeof check>> | null = null;

export function getUpdateStatus(): UpdateStatus {
  return updateStatus;
}

export function getUpdateInfo(): UpdateInfo | null {
  return updateInfo;
}

export function getDownloadProgress(): number | null {
  return downloadProgress;
}

export function getUpdateError(): string {
  return errorMessage;
}

export function dismissUpdate(): void {
  updateStatus = "idle";
  updateInfo = null;
  downloadProgress = null;
  errorMessage = "";
  pendingUpdate = null;
}

export async function checkForUpdates(): Promise<void> {
  updateStatus = "checking";
  errorMessage = "";
  downloadProgress = null;
  updateInfo = null;
  pendingUpdate = null;

  try {
    const update = await check();

    if (!update) {
      updateStatus = "upToDate";
      return;
    }

    pendingUpdate = update;
    updateInfo = {
      version: update.version,
      date: update.date ?? "",
      body: update.body ?? "",
    };
    updateStatus = "available";
  } catch (error) {
    errorMessage = error instanceof Error ? error.message : String(error);
    updateStatus = "error";
  }
}

export async function installUpdate(): Promise<void> {
  if (!pendingUpdate) {
    errorMessage = "No update available to install.";
    updateStatus = "error";
    return;
  }

  updateStatus = "downloading";
  downloadProgress = 0;

  try {
    let contentLength = 0;
    let downloaded = 0;

    await pendingUpdate.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          contentLength = (event.data as { contentLength?: number }).contentLength ?? 0;
          downloadProgress = 0;
          break;
        case "Progress": {
          const chunkLength = (event.data as { chunkLength?: number }).chunkLength ?? 0;
          downloaded += chunkLength;
          downloadProgress = contentLength > 0
            ? Math.min(99, Math.round((downloaded / contentLength) * 100))
            : null;
          break;
        }
        case "Finished":
          downloadProgress = 100;
          break;
      }
    });

    updateStatus = "readyToRestart";
  } catch (error) {
    errorMessage = error instanceof Error ? error.message : String(error);
    updateStatus = "error";
  }
}

export async function performRestart(): Promise<void> {
  try {
    await restartApp();
  } catch (error) {
    errorMessage = `Restart failed: ${error instanceof Error ? error.message : String(error)}`;
    updateStatus = "error";
  }
}
