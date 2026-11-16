import { expect, test as base, type Page } from "@playwright/test";
import type { MockBootstrap } from "../src/api/mock";

export const test = base;
export { expect };

/** Install the mock bootstrap before the app's first render. */
async function install(page: Page, bootstrap?: MockBootstrap): Promise<void> {
  if (!bootstrap) return;
  await page.addInitScript((data) => {
    (window as unknown as { __MOCK_BOOTSTRAP__?: unknown }).__MOCK_BOOTSTRAP__ =
      data;
  }, bootstrap);
}

/** Boot into the job list (the default account is already active). */
export async function openJobs(
  page: Page,
  bootstrap?: MockBootstrap,
): Promise<void> {
  await install(page, bootstrap);
  await page.goto("/");
  await expect(page.getByTestId("jobs-page")).toBeVisible();
}

/** Open a hash route directly, e.g. `/jobs/job-1-1/answer`. */
export async function openRoute(
  page: Page,
  route: string,
  bootstrap?: MockBootstrap,
): Promise<void> {
  await install(page, bootstrap);
  await page.goto(`/#${route}`);
}

export const MP3 = {
  name: "answer.mp3",
  mimeType: "audio/mpeg",
  buffer: Buffer.from("ID3-e2e-mock"),
};
