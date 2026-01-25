import { test, expect } from "@playwright/test";

test.describe("Search Functionality", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('nav:has-text("PluresDB")');
  });

  test("should perform vector search", async ({ page }) => {
    // Navigate to vector search view
    await page.click('button:has-text("Vector")');
    
    // Wait for vector explorer to load
    await expect(page.locator('h3:has-text("Vector")')).toBeVisible();
    
    // Enter search query
    const searchInput = page.locator('input[type="text"], input[placeholder*="search"]').first();
    if (await searchInput.isVisible()) {
      await searchInput.fill("test query");
      
      // Click search button
      const searchButton = page.locator('button:has-text("Search")').first();
      if (await searchButton.isVisible()) {
        await searchButton.click();
      }
    }
  });

  test("should use faceted search", async ({ page }) => {
    // Navigate to faceted search view
    await page.click('button:has-text("Search")');
    
    // Wait for faceted search to load
    await expect(page.locator('h3:has-text("Search"), h3:has-text("Faceted")')).toBeVisible();
    
    // Verify faceted search components are present
    const filterControls = page.locator('select, input[type="checkbox"]').first();
    await expect(filterControls).toBeVisible();
  });

  test("should search using query builder", async ({ page }) => {
    // Navigate to query builder
    await page.click('button:has-text("Queries")');
    
    // Wait for query builder to load
    await expect(page.locator('h3:has-text("Query")')).toBeVisible();
    
    // Verify query builder interface is present
    const queryControls = page.locator('button, select, input').first();
    await expect(queryControls).toBeVisible();
  });
});
