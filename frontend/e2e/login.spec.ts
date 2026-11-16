import { expect, openJobs, openRoute, test } from "./fixtures";

test.describe("登录与账号", () => {
  test("没有保存的账号时直接显示登录表单", async ({ page }) => {
    await openRoute(page, "/login", { accounts: [] });
    await expect(page.getByTestId("login-form")).toBeVisible();
    await expect(page.getByTestId("account-list")).toHaveCount(0);
  });

  test("登录成功后进入作业列表并显示当前账号", async ({ page }) => {
    await openRoute(page, "/login", { accounts: [] });
    await page.getByLabel("账号").fill("new_student");
    await page.getByLabel("密码").fill("secret");
    await page.getByTestId("login-submit").click();

    await expect(page.getByTestId("jobs-page")).toBeVisible();
    await expect(page.getByTestId("active-account")).toContainText(
      "new_student",
    );
  });

  test("登录失败显示错误，重试后成功", async ({ page }) => {
    await openRoute(page, "/login", {
      accounts: [],
      failOnStart: {
        account_login: {
          code: "network_error",
          message: "模拟登录失败",
          retryable: true,
        },
      },
    });
    await page.getByLabel("账号").fill("new_student");
    await page.getByLabel("密码").fill("secret");
    await page.getByTestId("login-submit").click();

    await expect(page.getByTestId("login-error")).toContainText("模拟登录失败");

    await page.getByTestId("login-submit").click();
    await expect(page.getByTestId("jobs-page")).toBeVisible();
  });

  test("切换账号后当前账号随之改变", async ({ page }) => {
    await openJobs(page);
    await page.getByTestId("nav-jobs").waitFor();
    await page.getByRole("link", { name: "切换账号" }).click();
    await expect(page.getByTestId("account-list")).toBeVisible();

    await page
      .getByTestId("account-row-acc-2")
      .getByRole("button", { name: "切换" })
      .click();

    await expect(page.getByTestId("jobs-page")).toBeVisible();
    await expect(page.getByTestId("active-account")).toContainText("林同学");
  });

  test("重新登录已保存的账号", async ({ page }) => {
    await openRoute(page, "/login");
    await page
      .getByTestId("account-row-acc-1")
      .getByRole("button", { name: "重新登录" })
      .click();

    await expect(page.getByTestId("login-form")).toContainText(
      "重新登录：陈同学",
    );
    await page.getByLabel("密码").fill("secret");
    await page.getByTestId("login-submit").click();

    await expect(page.getByTestId("jobs-page")).toBeVisible();
  });

  test("退出登录后回到登录页并标记会话失效", async ({ page }) => {
    await openJobs(page);
    await page.getByTestId("logout").click();

    await expect(page.getByTestId("account-list")).toBeVisible();
    await expect(page.getByTestId("account-row-acc-1")).toContainText(
      "需要重新登录",
    );
  });
});
