import { expect, openRoute, test } from "./fixtures";

test.describe("作业答案", () => {
  test("显示统计、逐题作答与原始 XML", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-1/answer");

    await expect(page.getByTestId("answer-page")).toBeVisible();
    await expect(page.getByTestId("answer-summary")).toBeVisible();
    await expect(page.getByTestId("question-card")).toHaveCount(8);
    await expect(page.getByTestId("question-card").first()).toContainText(
      "第 1 题",
    );
    await expect(page.getByText("原始试卷 XML")).toBeVisible();
    await expect(page.getByText(/任务 ID：content-job-1-1/)).toBeVisible();
  });

  test("返回作业列表", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-1/answer");
    await page.getByRole("link", { name: "返回作业" }).click();
    await expect(page.getByTestId("jobs-page")).toBeVisible();
  });
});
