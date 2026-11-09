import { expect, test, type Page } from "@playwright/test";
import { expectAccessible, open, startSession } from "./helpers";

/**
 * Documents in a session (T-019, T-017): picker, drag-and-drop, links,
 * indexing progress, failure with retry, removal. The native picker and OS
 * drop are checked by hand on each OS; here the mocked backend emits the same
 * events the Rust core would.
 */

type Hook = {
  files: { picked: string[] };
  sessionBus: { emit(kind: string, e: unknown): void };
};

async function pick(page: Page, paths: string[]) {
  await page.evaluate((p) => {
    (window as unknown as { __sidekickTest: Hook }).__sidekickTest.files.picked = p;
  }, paths);
}

async function drop(page: Page, paths: string[]) {
  await page.evaluate((p) => {
    (window as unknown as { __sidekickTest: Hook }).__sidekickTest.sessionBus.emit("drop", p);
  }, paths);
}

async function openLibrary(page: Page) {
  await open(page);
  await startSession(page);
  await page.getByRole("button", { name: /Documents \(/ }).click();
}

const docs = (page: Page) => page.getByRole("list", { name: "Documents" });

test("add files with the picker and watch them become ready", async ({ page }) => {
  await openLibrary(page);
  await pick(page, ["/tmp/pricing.pdf", "/tmp/notes.md"]);
  await page.getByRole("button", { name: "Add files" }).click();
  await expect(docs(page).getByText("pricing.pdf")).toBeVisible();
  await expect(docs(page).getByText("notes.md")).toBeVisible();
  await expect(docs(page).getByText(/Ready/)).toHaveCount(2);
  await expect(page.getByRole("button", { name: /Documents \(2\/2 ready\)/ })).toBeVisible();
});

test("dropping files on the window opens the library and indexes them", async ({ page }) => {
  await open(page);
  await startSession(page);
  // Collapsed on purpose: a drop opens the panel by itself.
  await expect(page.getByRole("button", { name: "Add files" })).toBeHidden();
  await drop(page, ["/Users/me/Desktop/roadmap.docx"]);
  await expect(page.getByRole("button", { name: "Add files" })).toBeVisible();
  await expect(docs(page).getByText("roadmap.docx")).toBeVisible();
  await expect(docs(page).getByText(/Ready/)).toBeVisible();
});

test("an unsupported file is refused with the supported types", async ({ page }) => {
  await openLibrary(page);
  await drop(page, ["/tmp/photo.heic"]);
  await expect(page.getByRole("alert")).toContainText("Supported: PDF, Word");
  await expect(docs(page)).toHaveCount(0);
});

test("a damaged file fails, can be retried, and can be removed", async ({ page }) => {
  await openLibrary(page);
  await drop(page, ["/tmp/broken.pdf"]);
  await expect(docs(page).getByText("Failed")).toBeVisible();
  await expect(page.locator("p.text-danger")).toContainText("the PDF is damaged");
  await page.getByRole("button", { name: "Retry" }).click();
  await expect(docs(page).getByText("Failed")).toBeVisible();
  await page.getByRole("button", { name: "Remove broken.pdf" }).click();
  await expect(docs(page)).toHaveCount(0);
});

test("links are added, validated, and removable", async ({ page }) => {
  await openLibrary(page);
  const add = page.getByRole("button", { name: "Add link" });
  await expect(add).toBeDisabled();
  await page.getByLabel("Link to add").fill("ftp://example.com/x");
  await add.click();
  await expect(page.getByRole("alert")).toContainText("Only http and https");
  await page.getByLabel("Link to add").fill("https://example.com/pricing");
  await add.click();
  await expect(docs(page).getByText("https://example.com/pricing")).toBeVisible();
  await expect(docs(page).getByText(/Ready/)).toBeVisible();
  await page.getByRole("button", { name: "Remove https://example.com/pricing" }).click();
  await expect(docs(page)).toHaveCount(0);
});

test("the library passes axe with documents and a failure showing", async ({ page }) => {
  await openLibrary(page);
  await drop(page, ["/tmp/a.pdf", "/tmp/broken.pdf"]);
  await expect(docs(page).getByText("Failed")).toBeVisible();
  await expectAccessible(page);
});
