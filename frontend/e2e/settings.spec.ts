import { expect, openRoute, test } from "./fixtures";

test.describe("设置", () => {
  test("展示当前账号与请求参数", async ({ page }) => {
    await openRoute(page, "/settings");
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();
    await expect(page.locator("#setting-concurrency")).toHaveValue("4");
    await expect(page.locator("#setting-timeout")).toHaveValue("15000");
    await expect(page.getByTestId("settings-account")).toContainText("陈同学");
  });

  test("修改并保存设置", async ({ page }) => {
    await openRoute(page, "/settings");
    await page.locator("#setting-concurrency").fill("8");
    await page.locator("#setting-keep-audio").uncheck();
    await page.getByTestId("settings-save").click();

    await expect(page.getByTestId("settings-saved")).toContainText("设置已保存");
    await expect(page.locator("#setting-concurrency")).toHaveValue("8");
    await expect(page.locator("#setting-keep-audio")).not.toBeChecked();
  });

  test("清理缓存在确认后执行", async ({ page }) => {
    await openRoute(page, "/settings");
    await page.getByTestId("settings-clear-cache").click();

    await expect(page.getByTestId("confirm-dialog")).toContainText(
      "确认清理本地缓存",
    );
    await page.getByTestId("confirm-cancel").click();
    await expect(page.getByTestId("cache-cleared")).toHaveCount(0);

    await page.getByTestId("settings-clear-cache").click();
    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("cache-cleared")).toContainText(
      "本地缓存已清理",
    );
  });
});
