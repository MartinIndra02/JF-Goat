import { expect, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauriMock";

test("autologin opens home directly", async ({ page }) => {
  await page.addInitScript(() => {
    (
      globalThis as { __TAURI_MOCK_AUTOLOGIN__?: boolean }
    ).__TAURI_MOCK_AUTOLOGIN__ = true;
  });

  await installTauriMock(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });

  await expect(
    page.getByRole("heading", { name: "Continue Watching" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "jfgoat" })).toHaveCount(0);
});
