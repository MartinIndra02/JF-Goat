import { expect, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauriMock";

test("toggling played state on a season propagates to its episodes", async ({ page }) => {
  await page.addInitScript(() => {
    (
      globalThis as { __TAURI_MOCK_AUTOLOGIN__?: boolean }
    ).__TAURI_MOCK_AUTOLOGIN__ = true;
  });

  await installTauriMock(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });

  // Navigate to Season 1 detail page
  await page.goto("/#/item?id=season-1", { waitUntil: "domcontentloaded" });

  await expect(page.getByRole("heading", { name: "Season 1" })).toBeVisible();

  // Verify that episodes are initially unwatched (no checkmark)
  const pilotButton = page.getByRole("button", { name: /Pilot/i });
  const nexusButton = page.getByRole("button", { name: /Nexus/i });

  await expect(pilotButton.locator(".bg-green-500\\/90")).toHaveCount(0);
  await expect(nexusButton.locator(".bg-green-500\\/90")).toHaveCount(0);

  // Click the Season's "Watched" toggle button to mark everything played
  const seasonWatchedButton = page.getByRole("button", { name: "Watched" });
  await seasonWatchedButton.click();

  // Assert that both episodes are immediately marked as watched (green checkmark container becomes visible)
  await expect(pilotButton.locator(".bg-green-500\\/90")).toBeVisible();
  await expect(nexusButton.locator(".bg-green-500\\/90")).toBeVisible();

  // Click the Season's "Watched" toggle button again to mark everything unplayed
  await seasonWatchedButton.click();

  // Assert that both episodes are marked unwatched again
  await expect(pilotButton.locator(".bg-green-500\\/90")).toHaveCount(0);
  await expect(nexusButton.locator(".bg-green-500\\/90")).toHaveCount(0);
});
