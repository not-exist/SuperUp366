import { expect, openJobs, test } from "./fixtures";

test.describe("作业列表", () => {
  test("渲染合并后的作业列表与首行字段", async ({ page }) => {
    await openJobs(page);

    await expect(page.getByTestId("jobs-summary")).toContainText("共 42 项");

    const first = page.getByTestId("job-row-job-1-1");
    await expect(first).toBeVisible();
    await expect(first).toContainText("Unit 1 词汇巩固");
    await expect(first).toContainText("高二(1)班");
    await expect(first).toContainText("已完成");
    await expect(first).toContainText("92 分");
  });

  test("按关键字搜索", async ({ page }) => {
    await openJobs(page);
    await page.locator("#job-search").fill("听力");

    await expect(page.getByTestId("jobs-summary")).toContainText(
      "筛选后 3 项",
    );
    await expect(page.getByTestId("job-row-job-1-3")).toBeVisible();
  });

  test("按状态筛选", async ({ page }) => {
    await openJobs(page);
    await page.locator("#job-status").selectOption("pending");

    await expect(page.getByTestId("jobs-summary")).toContainText(
      "筛选后 12 项",
    );
  });

  test("按类型筛选：单词 PK 与跟读", async ({ page }) => {
    await openJobs(page);

    await page.locator("#job-kind").selectOption("word_pk");
    await expect(page.getByTestId("jobs-summary")).toContainText(
      "筛选后 6 项",
    );
    await expect(page.getByTestId("job-row-job-1-7")).toContainText("单词 PK");

    await page.locator("#job-kind").selectOption("read_aloud");
    await expect(page.getByTestId("jobs-summary")).toContainText(
      "筛选后 3 项",
    );
    await expect(page.getByTestId("job-row-job-1-9")).toContainText("含朗读");
  });

  test("默写作业按类型筛选并提供提交入口", async ({ page }) => {
    await openJobs(page);

    await page.locator("#job-kind").selectOption("dictation");
    await expect(page.getByTestId("jobs-summary")).toContainText(
      "筛选后 3 项",
    );
    await expect(page.getByTestId("job-row-job-1-2")).toContainText("默写线索");

    await page.getByTestId("job-row-job-1-2").click();
    await expect(page.getByTestId("answer-page")).toBeVisible();
    await expect(page.getByTestId("answer-writing-link")).toHaveCount(0);
    await expect(page.getByTestId("answer-dictation-link")).toContainText(
      "默写作业",
    );
    await page.getByRole("link", { name: "去提交默写" }).click();
    await expect(page.getByTestId("dictation-page")).toBeVisible();
  });

  test("组合筛选与清除筛选", async ({ page }) => {
    await openJobs(page);
    await page.locator("#job-search").fill("单词卡");
    await page.locator("#job-status").selectOption("finished");

    await expect(page.getByTestId("jobs-summary")).toContainText(
      "筛选后 6 项",
    );

    await page.locator("#job-search").fill("不存在的作业");
    await expect(page.getByText("没有匹配的作业")).toBeVisible();

    await page.getByRole("button", { name: "清除筛选" }).click();
    await expect(page.getByTestId("jobs-summary")).toContainText("共 42 项");
  });

  test("点击作业行进入答案页", async ({ page }) => {
    await openJobs(page);
    await page.getByTestId("job-row-job-1-1").click();

    await expect(page.getByTestId("answer-page")).toBeVisible();
    await expect(page.getByRole("heading", { level: 1 })).toContainText(
      "Unit 1 词汇巩固",
    );
  });
});
