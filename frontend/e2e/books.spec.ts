import { expect, MP3, openRoute, test } from "./fixtures";

test.describe("书籍", () => {
  test("书架列出所有书籍", async ({ page }) => {
    await openRoute(page, "/books");
    await expect(page.getByTestId("book-grid")).toBeVisible();
    await expect(page.locator('[data-testid^="book-card-"]')).toHaveCount(3);
    await expect(page.getByTestId("book-card-book-1")).toContainText(
      "英语必修 第一册",
    );
    await expect(page.getByTestId("book-card-book-3")).toContainText(
      "2025 阅读器",
    );
  });

  test("书籍详情按章节展示作业", async ({ page }) => {
    await openRoute(page, "/books");
    await page.getByTestId("book-card-book-1").click();

    await expect(page.getByTestId("book-hero")).toBeVisible();
    await expect(page.getByTestId("chapter")).toHaveCount(3);
    await expect(page.locator('[data-testid^="book-job-"]')).toHaveCount(12);
    await expect(page.getByTestId("book-hero")).toContainText("已完成");
  });

  test("打开只读的普通作业", async ({ page }) => {
    await openRoute(page, "/books/book-1?type=1&course=course-1");
    await page.getByTestId("book-job-book-1-c1-j1").click();

    await expect(page.getByTestId("book-question")).toHaveCount(4);
    await expect(page.getByText("作业 · 只读查看")).toBeVisible();
    await page.getByRole("link", { name: "返回书籍" }).click();
    await expect(page.getByTestId("book-hero")).toBeVisible();
  });

  test("从书籍进入朗读提交并返回书籍", async ({ page }) => {
    await openRoute(page, "/books/book-1?type=1&course=course-1");
    await page.getByTestId("book-job-book-1-c1-j2").click();

    await expect(page.getByTestId("readaloud-page")).toBeVisible();
    await expect(page.getByTestId("readaloud-slot-1")).toBeVisible();
    await expect(page.getByRole("link", { name: "返回书籍" })).toBeVisible();

    for (let slot = 1; slot <= 5; slot += 1) {
      const chooser = page.waitForEvent("filechooser");
      await page.getByTestId(`readaloud-pick-${slot}`).click();
      await (await chooser).setFiles(MP3);
      await expect(
        page.getByTestId(`readaloud-status-${slot}`),
      ).toContainText("已上传");
    }

    await page.getByTestId("readaloud-preview").click();
    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("readaloud-result")).toContainText("提交成功");

    await page.getByRole("link", { name: "返回书籍" }).click();
    await expect(page.getByTestId("book-hero")).toBeVisible();
  });

  test("打开只读的单词任务", async ({ page }) => {
    await openRoute(page, "/books/book-1?type=1&course=course-1");
    await page.getByTestId("book-job-book-1-c1-j4").click();

    await expect(page.getByTestId("book-word")).toHaveCount(4);
    await expect(page.getByText("单词任务 · 只读查看")).toBeVisible();
  });

  test("没有书籍时显示空状态", async ({ page }) => {
    await openRoute(page, "/books", { books: [] });
    await expect(page.getByText("暂无书籍")).toBeVisible();
  });
});
