import { expect, openJobs, openRoute, test } from "./fixtures";

test.describe("应用文写作", () => {
  test("按类型筛选并进入应用文页面", async ({ page }) => {
    await openJobs(page);

    await page.locator("#job-kind").selectOption("applied_writing");
    await expect(page.getByTestId("jobs-summary")).toContainText("筛选后 3 项");
    await expect(page.getByTestId("job-row-job-1-8")).toContainText("应用文");

    await page.getByTestId("job-row-job-1-8").click();
    await expect(page.getByTestId("answer-page")).toBeVisible();
    await expect(page.getByTestId("answer-writing-link")).toContainText(
      "应用文写作",
    );
    await page.getByRole("link", { name: "去提交应用文" }).click();
    await expect(page.getByTestId("appliedwriting-page")).toBeVisible();
    await expect(page.getByRole("heading", { level: 1 })).toContainText(
      "应用文写作：My Weekend",
    );
    await expect(page.getByTestId("writing-question-1")).toContainText(
      "假定你是李华",
    );
    await expect(page.getByTestId("writing-preview")).toBeDisabled();
  });

  test("填写作文、预览 XML 并提交一次", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-8/writing");

    const input = page.getByTestId("writing-input-1");
    await input.fill("Dear Chris,\nI took part in a basketball game last week.");
    await expect(page.getByTestId("writing-wordcount-1")).toContainText(
      "已输入单词：11",
    );
    await expect(page.getByTestId("writing-wordcount-1")).toContainText(
      "字符：54 / 3000",
    );

    await page.getByTestId("writing-preview").click();
    await expect(page.getByTestId("confirm-dialog")).toBeVisible();
    await expect(page.getByTestId("writing-preview-xml")).toContainText(
      "<![CDATA[Dear Chris,",
    );

    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("writing-result")).toContainText("提交成功");
    await expect(page.getByTestId("writing-result")).toContainText("当前分数 12");
    await expect(page.getByTestId("writing-preview")).toBeDisabled();
  });

  test("答案页为纯应用文作业提供提交入口", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-8/answer");

    await expect(page.getByTestId("answer-writing-link")).toContainText(
      "应用文写作",
    );
    await page.getByRole("link", { name: "去提交应用文" }).click();
    await expect(page.getByTestId("appliedwriting-page")).toBeVisible();
  });

  test("提交后作业列表显示已完成", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-8/writing");

    await page.getByTestId("writing-input-1").fill("Dear Chris, thank you.");
    await page.getByTestId("writing-preview").click();
    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("writing-result")).toBeVisible();

    await page.getByRole("link", { name: "返回作业" }).click();
    await expect(page.getByTestId("jobs-page")).toBeVisible();
    await expect(page.getByTestId("job-row-job-1-8")).toContainText("已完成");
  });
});
