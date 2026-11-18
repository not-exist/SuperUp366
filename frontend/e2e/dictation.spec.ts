import { expect, openRoute, test } from "./fixtures";

test.describe("默写作业", () => {
  test("从答案页进入默写页，填写两题、预览 XML 并提交", async ({ page }) => {
    await openRoute(page, "/jobs/job-2-2/answer");
    await expect(page.getByTestId("answer-dictation-link")).toContainText(
      "默写作业",
    );

    await page.getByRole("link", { name: "去提交默写" }).click();
    await expect(page.getByTestId("dictation-page")).toBeVisible();
    await expect(page.getByRole("heading", { level: 1 })).toContainText(
      "Unit 2 课文默写练习",
    );
    await expect(page.getByTestId("dictation-question-1")).toContainText(
      "篇章默写",
    );
    await expect(page.getByTestId("dictation-question-2")).toContainText(
      "句子默写",
    );
    await expect(page.getByTestId("dictation-preview")).toBeDisabled();

    await page
      .getByTestId("dictation-input-1")
      .fill("The teacher asked us to read the passage aloud before class.");
    await page
      .getByTestId("dictation-input-2")
      .fill("The more you practice, the more confident you will become.");
    await expect(page.getByTestId("dictation-progress")).toContainText(
      "已填写 2 / 2 题",
    );

    await page.getByTestId("dictation-preview").click();
    await expect(page.getByTestId("confirm-dialog")).toBeVisible();
    await expect(page.getByTestId("dictation-preview-xml")).toContainText(
      "<![CDATA[The teacher asked us",
    );

    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("dictation-result")).toContainText("提交成功");
    await expect(page.getByTestId("dictation-result")).toContainText(
      "当前分数 18",
    );
    await expect(page.getByTestId("dictation-preview")).toBeDisabled();
    await expect(page.getByTestId("dictation-input-1")).toBeDisabled();
  });

  test("支持重做的作业显示重新提交文案", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-2/dictation");

    await expect(page.getByTestId("dictation-page")).toBeVisible();
    await expect(page.getByTestId("dictation-retest-notice")).toContainText(
      "该作业支持重做",
    );
    await expect(page.getByTestId("dictation-preview")).toContainText(
      "预览并重新提交",
    );
    await expect(page.getByTestId("dictation-preview")).toBeDisabled();
    await expect(page.getByTestId("dictation-input-1")).toBeEnabled();
  });
});
