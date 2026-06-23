import { expect, test } from "@playwright/test";
import { installTauriMock } from "./fixtures/tauriMock";

test("connect -> login -> home -> detail -> player -> search with propagation", async ({
  page,
}) => {
  await installTauriMock(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });

  await expect(page.getByRole("heading", { name: "JF Goat" })).toBeVisible();

  await page
    .getByPlaceholder("http://your-server:8096")
    .fill("http://demo.local");
  await page.getByRole("button", { name: "Connect" }).click();

  await expect(page.getByRole("heading", { name: "Sign In" })).toBeVisible();
  await page.getByPlaceholder("Username").fill("demo");
  await page.getByPlaceholder("Password").fill("password");
  await page.getByRole("button", { name: "Sign In" }).click();

  await expect(
    page.getByRole("heading", { name: "Continue Watching" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open details for Pilot" }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Open details for Pilot" }).click();
  await expect(
    page.getByRole("heading", { name: /S1 - E1 - Pilot/ }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Resume" }).click();

  await expect(page.getByLabel("Close player")).toBeVisible();
  await expect(page.getByText("My Show - S1 E1 - Pilot").first()).toBeVisible();

  await page.getByLabel("Close player").click();
  await page.getByLabel("Go back").click();

  await expect(
    page.getByRole("button", { name: "Open details for Pilot" }),
  ).toHaveCount(0);

  await page.getByPlaceholder("Search your library...").fill("pilot");
  await expect(
    page.getByRole("heading", { name: "Episodes (1)" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Open details for Pilot" }),
  ).toBeVisible();
});
