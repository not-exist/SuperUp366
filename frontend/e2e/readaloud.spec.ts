import { expect, MP3, openRoute, test } from "./fixtures";

test.describe("朗读跟读", () => {
  test.beforeEach(async ({ page }) => {
    await openRoute(page, "/jobs/job-1-9/readaloud");
    await expect(page.getByTestId("readaloud-page")).toBeVisible();
  });

  test("展示自动识别的题目并禁用未就绪的提交", async ({ page }) => {
    await expect(page.getByTestId("readaloud-slot-1")).toBeVisible();
    await expect(page.getByTestId("readaloud-slot-5")).toBeVisible();
    await expect(page.getByText("已上传 0 / 5")).toBeVisible();
    await expect(page.getByTestId("readaloud-preview")).toBeDisabled();
  });

  test("非 mp3 文件被拒绝", async ({ page }) => {
    const chooser = page.waitForEvent("filechooser");
    await page.getByTestId("readaloud-pick-1").click();
    await (await chooser).setFiles({
      name: "notes.txt",
      mimeType: "text/plain",
      buffer: Buffer.from("not audio"),
    });

    await expect(page.getByTestId("readaloud-status-1")).toContainText("失败");
    await expect(page.getByText("只接受 mp3 文件")).toBeVisible();
    await expect(page.getByText("已上传 0 / 5")).toBeVisible();
  });

  test("上传全部录音后预览并提交", async ({ page }) => {
    for (let slot = 1; slot <= 5; slot += 1) {
      const chooser = page.waitForEvent("filechooser");
      await page.getByTestId(`readaloud-pick-${slot}`).click();
      await (await chooser).setFiles(MP3);
      await expect(
        page.getByTestId(`readaloud-status-${slot}`),
      ).toContainText("已上传");
    }

    await expect(page.getByText("已上传 5 / 5")).toBeVisible();
    await expect(page.getByTestId("readaloud-preview")).toBeEnabled();

    await page.getByTestId("readaloud-preview").click();
    await expect(page.getByTestId("confirm-dialog")).toContainText(
      "确认提交朗读成绩",
    );
    await page.getByTestId("confirm-submit").click();

    await expect(page.getByTestId("readaloud-result")).toContainText(
      "提交成功",
    );
  });

  test("取消确认后可以重新预览", async ({ page }) => {
    for (let slot = 1; slot <= 5; slot += 1) {
      const chooser = page.waitForEvent("filechooser");
      await page.getByTestId(`readaloud-pick-${slot}`).click();
      await (await chooser).setFiles(MP3);
      await expect(
        page.getByTestId(`readaloud-status-${slot}`),
      ).toContainText("已上传");
    }

    await page.getByTestId("readaloud-preview").click();
    await expect(page.getByTestId("confirm-dialog")).toBeVisible();
    await page.getByTestId("confirm-cancel").click();
    await expect(page.getByTestId("confirm-dialog")).toHaveCount(0);
    await expect(page.getByTestId("readaloud-preview")).toBeEnabled();

    await page.getByTestId("readaloud-preview").click();
    await expect(page.getByTestId("confirm-dialog")).toBeVisible();
  });
});

test.describe("朗读重新提交", () => {
  test("已提交的任务可以重新提交", async ({ page }) => {
    await openRoute(page, "/jobs/job-1-9/readaloud", {
      submittedReadAloud: ["job-1-9"],
    });
    await expect(page.getByTestId("readaloud-page")).toBeVisible();
    await expect(page.getByText("已提交 · 可重新提交")).toBeVisible();
    await expect(page.getByTestId("readaloud-pick-1")).toBeEnabled();

    for (let slot = 1; slot <= 5; slot += 1) {
      const chooser = page.waitForEvent("filechooser");
      await page.getByTestId(`readaloud-pick-${slot}`).click();
      await (await chooser).setFiles(MP3);
      await expect(
        page.getByTestId(`readaloud-status-${slot}`),
      ).toContainText("已上传");
    }

    await page.getByTestId("readaloud-preview").click();
    await expect(page.getByTestId("confirm-dialog")).toContainText(
      "确认重新提交朗读成绩",
    );
    await expect(page.getByTestId("confirm-dialog")).toContainText("累加");
    await page.getByTestId("confirm-submit").click();
    await expect(page.getByTestId("readaloud-result")).toContainText(
      "重新提交成功",
    );
  });
});
