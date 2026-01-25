import { test, expect } from "@playwright/test";

test.describe("Import/Export", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('nav:has-text("PluresDB")');
  });

  test("should navigate to import/export view", async ({ page }) => {
    await page.click('button:has-text("Import/Export")');
    
    // Verify import/export interface is loaded
    await expect(page.locator('h3:has-text("Import"), h3:has-text("Export")')).toBeVisible();
  });

  test("should show export tab", async ({ page }) => {
    await page.click('button:has-text("Import/Export")');
    
    // Click export tab if there are tabs
    const exportTab = page.locator('button:has-text("Export")').first();
    if (await exportTab.isVisible()) {
      await exportTab.click();
      await expect(exportTab).toHaveAttribute("aria-pressed", "true");
    }
  });

  test("should show import tab", async ({ page }) => {
    await page.click('button:has-text("Import/Export")');
    
    // Click import tab
    const importTab = page.locator('button:has-text("Import")').first();
    if (await importTab.isVisible()) {
      await importTab.click();
      await expect(importTab).toHaveAttribute("aria-pressed", "true");
    }
  });

  test("should show examples tab", async ({ page }) => {
    await page.click('button:has-text("Import/Export")');
    
    // Click examples tab
    const examplesTab = page.locator('button:has-text("Examples")').first();
    if (await examplesTab.isVisible()) {
      await examplesTab.click();
      await expect(examplesTab).toHaveAttribute("aria-pressed", "true");
      
      // Verify example datasets are shown
      await expect(page.locator('text=Example Datasets')).toBeVisible();
    }
  });

  test("should display example datasets", async ({ page }) => {
    await page.click('button:has-text("Import/Export")');
    
    // Navigate to examples tab
    const examplesTab = page.locator('button:has-text("Examples")').first();
    if (await examplesTab.isVisible()) {
      await examplesTab.click();
      
      // Verify dataset cards are visible (check each separately)
      await expect(page.locator('text=User Profiles')).toBeVisible();
      await expect(page.locator('text=E-Commerce Products')).toBeVisible();
      await expect(page.locator('text=Social Graph')).toBeVisible();
      await expect(page.locator('text=Document Collection')).toBeVisible();
    }
  });
});
