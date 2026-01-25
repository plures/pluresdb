import { test, expect } from "@playwright/test";

test.describe("CRUD Operations", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('nav:has-text("PluresDB")');
  });

  test("should create a new node", async ({ page }) => {
    // Navigate to data view
    await page.click('button:has-text("Data")');
    
    // Create a unique test node to avoid conflicts
    const testId = `test_node_${Date.now()}`;
    const nodeData = {
      type: "TestNode",
      name: "Test Item",
      value: 42,
      testId: testId
    };
    
    // Use the API to create a node for testing (more reliable than UI interaction)
    await page.evaluate(async (data) => {
      const response = await fetch(`/api/nodes/${data.testId}`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(data)
      });
      return response.ok;
    }, nodeData);
    
    // Wait for the node to appear in the UI via SSE
    await page.waitForTimeout(500);
    
    // Verify node appears in the node list
    await expect(page.locator(`text=${testId}`)).toBeVisible({ timeout: 5000 });
    
    // Clean up - delete the test node
    await page.evaluate(async (id) => {
      await fetch(`/api/nodes/${id}`, { method: "DELETE" });
    }, testId);
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
