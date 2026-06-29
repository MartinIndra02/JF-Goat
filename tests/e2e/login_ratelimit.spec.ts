import { expect, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauriMock";

test("login rate limiting error is displayed to user", async ({ page }) => {
  // Set the mock rate limit flag
  await page.addInitScript(() => {
    (globalThis as any).__TAURI_MOCK_LOGIN_RATE_LIMIT__ = true;
  });

  await installTauriMock(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });

  // Connect to server first
  await page
    .getByPlaceholder("http://your-server:8096")
    .fill("http://demo.local");
  await page.getByRole("button", { name: "Connect" }).click();

  // Try signing in
  await expect(page.getByRole("heading", { name: "Sign In" })).toBeVisible();
  await page.getByPlaceholder("Username").fill("demo");
  await page.getByPlaceholder("Password").fill("password");
  await page.getByRole("button", { name: "Sign In" }).click();

  // The toast showing the rate limit error should appear
  await expect(page.getByText(/Too many login attempts/i)).toBeVisible();
});
