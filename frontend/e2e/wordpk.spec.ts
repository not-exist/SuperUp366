import { expect, openRoute, test } from "./fixtures";

test.describe("单词 PK", () => {
  test.beforeEach(async ({ page }) => {
    await openRoute(page, "/jobs/job-1-7/wordpk");
    await expect(page.getByTestId("wordpk-page")).toBeVisible();
  });

  test("输入非法时禁止抽词", async ({ page }) => {
    await page.locator("#wordpk-score").fill("20");
    await page.locator("#wordpk-total").fill("10");

    await expect(page.getByTestId("wordpk-draw")).toBeDisabled();
    await expect(page.getByText("分数需为 0 到 total 之间的整数")).toBeVisible();
  });

  test("抽词后按分数展示正确与错误单词", async ({ page }) => {
    await page.locator("#wordpk-score").fill("8");
    await page.locator("#wordpk-total").fill("10");
    await page.getByTestId("wordpk-draw").click();

    await expect(page.getByTestId("word-card")).toHaveCount(10);
    await expect(page.locator(".word-card--known")).toHaveCount(8);
    await expect(page.getByTestId("wordpk-summary")).toContainText("词库可用");
  });

  test("词库不足时显示服务端错误", async ({ page }) => {
    await page.locator("#wordpk-score").fill("8");
    await page.locator("#wordpk-total").fill("20");
    await page.getByTestId("wordpk-draw").click();

    await expect(page.getByTestId("wordpk-draw-error")).toContainText(
      "词库只有 12 个可用单词",
    );
    await expect(page.getByTestId("word-card")).toHaveCount(0);
  });

  test("预览后取消再提交", async ({ page }) => {
    await page.getByTestId("wordpk-draw").click();
    await expect(page.getByTestId("word-card")).toHaveCount(10);

    await page.getByTestId("wordpk-submit").click();
    await expect(page.getByTestId("confirm-dialog")).toBeVisible();
    await expect(page.getByTestId("confirm-dialog")).toContainText(
      "将提交 10 个单词",
    );
    await page.getByTestId("confirm-cancel").click();
    await expect(page.getByTestId("confirm-dialog")).toHaveCount(0);

    await page.getByTestId("wordpk-submit").click();
    await page.getByTestId("confirm-submit").click();

    await expect(page.getByTestId("wordpk-result")).toContainText("提交成功");
    await expect(page.getByTestId("wordpk-submit")).toBeDisabled();
  });

  test("重新抽取会重置上一次结果", async ({ page }) => {
    await page.getByTestId("wordpk-draw").click();
    await expect(page.getByTestId("word-card")).toHaveCount(10);

    await page.locator("#wordpk-score").fill("3");
    await page.locator("#wordpk-total").fill("5");
    await page.getByTestId("wordpk-draw").click();
    await expect(page.getByTestId("word-card")).toHaveCount(5);
  });
});
