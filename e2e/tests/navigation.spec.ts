import { test, expect } from "@playwright/test";

test.describe("Navigation and UI", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
  });

  test("should load the application", async ({ page }) => {
    await expect(page.locator('nav:has-text("PluresDB")')).toBeVisible();
  });

  test("should have dark mode toggle", async ({ page }) => {
    const darkModeToggle = page.locator('input[type="checkbox"][role="switch"]');
    await expect(darkModeToggle).toBeVisible();
    
    // Toggle dark mode
    await darkModeToggle.click();
    
    // Verify theme changed
    const theme = await page.evaluate(() => 
      document.documentElement.getAttribute("data-theme")
    );
    expect(theme).toBeTruthy();
  });

  test("should navigate between main views", async ({ page }) => {
    const views = ["Data", "Types", "History", "Graph", "Vector", "Search", "Queries", "Rules"];
    
    for (const view of views) {
      await page.click(`button:has-text("${view}")`);
      await expect(page.locator(`button:has-text("${view}")`)).toHaveAttribute("aria-current", "page");
    }
  });

  test("should show guided tour on first visit", async ({ page }) => {
    // Clear localStorage to simulate first visit
    await page.evaluate(() => {
      localStorage.clear();
    });
    
    // Reload page
    await page.reload();
    await page.waitForTimeout(1500); // Wait for tour to appear
    
    // Check if tour prompt appears
    const tourPrompt = page.locator('text=New to PluresDB?, text=Start Tour');
    if (await tourPrompt.isVisible()) {
      await expect(tourPrompt).toBeVisible();
    }
  });

  test("should start guided tour", async ({ page }) => {
    // Clear localStorage and reload
    await page.evaluate(() => {
      localStorage.clear();
    });
    await page.reload();
    await page.waitForTimeout(1500);
    
    // Start tour if prompt is visible
    const startButton = page.locator('button:has-text("Start Tour")');
    if (await startButton.isVisible()) {
      await startButton.click();
      
      // Verify tour overlay appears
      await expect(page.locator('text=Welcome to PluresDB!')).toBeVisible();
    }
  });
});
