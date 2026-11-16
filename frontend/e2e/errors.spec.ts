import { expect, openJobs, openRoute, test } from "./fixtures";

test.describe("错误与空状态", () => {
  test("账号列表失败时显示启动错误并可重试", async ({ page }) => {
    await openRoute(page, "/", {
      failAlways: {
        accounts_list: {
          code: "network_error",
          message: "账号服务不可用",
          retryable: true,
        },
      },
    });

    await expect(page.getByText("无法读取账号")).toBeVisible();
    await expect(page.getByText("错误码：network_error")).toBeVisible();

    await page.evaluate(() => window.__mock?.clearFailures());
    await page.getByRole("button", { name: /重试/ }).click();
    await expect(page.getByTestId("jobs-page")).toBeVisible();
  });

  test("作业列表可重试错误", async ({ page }) => {
    await openJobs(page, {
      failAlways: {
        jobs_list: {
          code: "network_error",
          message: "作业服务不可用",
          retryable: true,
        },
      },
    });

    await expect(page.getByText("加载失败")).toBeVisible();
    await expect(page.getByText("作业服务不可用")).toBeVisible();

    await page.evaluate(() => window.__mock?.clearFailures());
    await page.getByRole("button", { name: /重试/ }).click();
    await expect(page.getByTestId("job-row-job-1-1")).toBeVisible();
  });

  test("不可重试的错误不显示重试按钮", async ({ page }) => {
    await openJobs(page, {
      failAlways: {
        jobs_list: {
          code: "protocol_drift",
          message: "响应格式已变化",
          retryable: false,
        },
      },
    });

    await expect(page.getByText("加载失败")).toBeVisible();
    await expect(page.getByText("错误码：protocol_drift")).toBeVisible();
    await expect(page.getByRole("button", { name: /重试/ })).toHaveCount(0);
  });

  test("没有作业时显示空状态", async ({ page }) => {
    await openJobs(page, { jobs: [] });
    await expect(page.getByText("暂无作业")).toBeVisible();
    await expect(page.getByTestId("jobs-summary")).toContainText("共 0 项");
  });
});
