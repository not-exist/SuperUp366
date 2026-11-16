import { expect, openRoute, test } from "./fixtures";

test.describe("错题本", () => {
  test("读取总览并抽题预览后提交", async ({ page }) => {
    await openRoute(page, "/wrong-notes");
    await expect(page.getByTestId("wrongnote-page")).toBeVisible();
    await expect(page.getByTestId("correction-remaining")).toContainText(
      "未清理 12 题",
    );

    await page.getByTestId("correction-draw").click();
    await expect(page.getByTestId("confirm-dialog")).toContainText("提交订正");
    await expect(page.getByTestId("confirm-dialog")).toContainText(
      "订正 6 道题",
    );

    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("correction-result")).toContainText(
      "已提交 6 道题",
    );
    await expect(page.getByTestId("correction-result")).toContainText(
      "批改完成",
    );
    await expect(page.getByTestId("correction-remaining")).toContainText(
      "未清理 6 题",
    );
  });

  test("取消确认后从预览区重新提交", async ({ page }) => {
    await openRoute(page, "/wrong-notes");
    await page.getByTestId("correction-draw").click();
    await expect(page.getByTestId("confirm-dialog")).toBeVisible();

    await page.getByTestId("confirm-cancel").click();
    await expect(page.getByTestId("confirm-dialog")).toHaveCount(0);
    await expect(page.getByTestId("correction-preview")).toBeVisible();

    await page.getByTestId("correction-submit").click();
    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("correction-result")).toContainText(
      "已提交",
    );
  });

  test("没有错题时提示已全部订正", async ({ page }) => {
    await openRoute(page, "/wrong-notes", { correctionRemaining: 0 });
    await expect(page.getByTestId("correction-remaining")).toContainText(
      "未清理 0 题",
    );

    await page.getByTestId("correction-draw").click();
    await expect(page.getByText("没有需要订正的题目")).toBeVisible();
    await expect(page.getByTestId("confirm-dialog")).toHaveCount(0);
  });

  test("调整抽题数量", async ({ page }) => {
    await openRoute(page, "/wrong-notes");
    await page.getByTestId("correction-count").fill("3");
    await page.getByTestId("correction-draw").click();

    await expect(page.getByTestId("confirm-dialog")).toContainText(
      "订正 3 道题",
    );
  });
});
