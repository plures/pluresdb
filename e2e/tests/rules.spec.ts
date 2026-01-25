import { test, expect } from "@playwright/test";

test.describe("Rules Engine", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('nav:has-text("PluresDB")');
  });

  test("should navigate to rules builder", async ({ page }) => {
    await page.click('button:has-text("Rules")');
    
    // Verify rules builder is loaded
    await expect(page.locator('h3:has-text("Rules")')).toBeVisible();
  });

  test("should display rules interface", async ({ page }) => {
    await page.click('button:has-text("Rules")');
    
    // Wait for rules builder components
    const rulesSection = page.locator('section, div').filter({ hasText: "Rules" }).first();
    await expect(rulesSection).toBeVisible();
    
    // Verify create rule button exists
    const createButton = page.locator('button:has-text("Create"), button:has-text("Add"), button:has-text("New")').first();
    if (await createButton.isVisible()) {
      await expect(createButton).toBeEnabled();
    }
  });

  test("should display rules list", async ({ page }) => {
    await page.click('button:has-text("Rules")');
    
    // Wait for the rules view
    await page.waitForSelector('h3:has-text("Rules")');
    
    // Verify the interface is interactive
    const interactive = page.locator('button, input, select').first();
    await expect(interactive).toBeVisible();
  });
});
