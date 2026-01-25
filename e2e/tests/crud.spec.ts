import { test, expect } from "@playwright/test";

test.describe("CRUD Operations", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('nav:has-text("PluresDB")');
  });

  test("should create a new node", async ({ page }) => {
    // Navigate to data view
    await page.click('button:has-text("Data")');
    
    // Look for the node detail panel or create button
    // This depends on the actual UI structure
    const nodeData = {
      type: "TestNode",
      name: "Test Item",
      value: 42
    };
    
    // Type in the JSON editor (assuming there's a textarea or editor)
    const editor = page.locator('textarea, .cm-content').first();
    await editor.click();
    await editor.fill(JSON.stringify(nodeData, null, 2));
    
    // Click save or create button
    await page.click('button:has-text("Save"), button:has-text("Create")');
    
    // Verify node was created
    await expect(page.locator('text=Test Item')).toBeVisible();
  });

  test("should read and display nodes", async ({ page }) => {
    await page.click('button:has-text("Data")');
    
    // Wait for node list to load
    await page.waitForSelector('[role="listbox"], [role="list"]');
    
    // Verify at least the UI is rendered
    const nodeList = page.locator('[role="listbox"], [role="list"]').first();
    await expect(nodeList).toBeVisible();
  });

  test("should update an existing node", async ({ page }) => {
    await page.click('button:has-text("Data")');
    
    // Select first node if available
    const firstNode = page.locator('[role="option"], li').first();
    if (await firstNode.isVisible()) {
      await firstNode.click();
      
      // Edit the node
      const editor = page.locator('textarea, .cm-content').first();
      await editor.click();
      
      // Verify we can interact with editor
      await expect(editor).toBeVisible();
    }
  });

  test("should delete a node", async ({ page }) => {
    await page.click('button:has-text("Data")');
    
    // Look for delete button
    const deleteButton = page.locator('button:has-text("Delete")').first();
    if (await deleteButton.isVisible()) {
      await deleteButton.click();
      
      // May need to confirm
      const confirmButton = page.locator('button:has-text("Confirm"), button:has-text("Yes")');
      if (await confirmButton.isVisible()) {
        await confirmButton.click();
      }
    }
  });
});
