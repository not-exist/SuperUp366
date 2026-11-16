import { expect, openJobs, openRoute, test } from "./fixtures";

test.describe("导航", () => {
  test("侧边栏在四个页面间切换并标记当前项", async ({ page }) => {
    await openJobs(page);

    await page.getByTestId("nav-books").click();
    await expect(page.getByTestId("book-grid")).toBeVisible();
    await expect(page.getByTestId("nav-books")).toHaveClass(
      /nav-link--active/,
    );

    await page.getByTestId("nav-wrong-notes").click();
    await expect(page.getByTestId("wrongnote-page")).toBeVisible();

    await page.getByTestId("nav-settings").click();
    await expect(page.getByRole("heading", { name: "设置" })).toBeVisible();

    await page.getByTestId("nav-jobs").click();
    await expect(page.getByTestId("jobs-page")).toBeVisible();
  });

  test("未知路由回到作业列表", async ({ page }) => {
    await openRoute(page, "/does-not-exist");
    await expect(page.getByTestId("jobs-page")).toBeVisible();
  });

  test("没有活动账号时任何页面都回到登录页", async ({ page }) => {
    await openRoute(page, "/books", { accounts: [] });
    await expect(page.getByTestId("login-form")).toBeVisible();
  });
});
