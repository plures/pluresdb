<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";

  let activeTab: "import" | "export" = "export";
  let exportFormat: "json" | "csv" = "json";
  let importFormat: "json" | "csv" = "json";
  let exportType = "";
  let exportDataText = "";
  let importDataText = "";
  let importMapping: Record<string, string> = {};
  let availableFields: string[] = [];
  let loading = false;
  let dark = false;

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  onMount(() => {
    loadTypes();
  });

  async function loadTypes() {
    try {
      const res = await fetch("/api/list");
      const nodes = await res.json();

      // Extract unique field names from all nodes
      const fields = new Set<string>();
      for (const node of nodes) {
        Object.keys(node.data || {}).forEach((field) => fields.add(field));
      }
      availableFields = Array.from(fields).sort();
    } catch (error) {
      console.error("Error loading types:", error);
    }
  }

  async function performExport() {
    if (!exportType.trim()) {
      toast("Please select a type to export", "error");
      return;
    }

    loading = true;
    try {
      const res = await fetch(`/api/instances?type=${encodeURIComponent(exportType)}`);
      const nodes = await res.json();

      if (exportFormat === "json") {
        exportDataText = JSON.stringify(nodes, null, 2);
      } else {
        // CSV export
        if (nodes.length === 0) {
          exportDataText = "No data to export";
        } else {
          const fields = ["id", ...Object.keys(nodes[0].data || {})];
          const csvHeader = fields.join(",");
          const csvRows = nodes.map((node) =>
            fields
              .map((field) => {
                const value = field === "id" ? node.id : node.data[field] || "";
                return `"${String(value).replace(/"/g, '""')}"`;
              })
              .join(","),
          );
          exportDataText = [csvHeader, ...csvRows].join("\n");
        }
      }

      toast(`Exported ${nodes.length} records`, "success");
    } catch (error) {
      toast("Export failed", "error");
      console.error("Error exporting:", error);
    } finally {
      loading = false;
    }
  }

  function downloadExport() {
    if (!exportDataText) return;

    const blob = new Blob([exportDataText], {
      type: exportFormat === "json" ? "application/json" : "text/csv",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `pluresdb-export-${exportType}-${Date.now()}.${exportFormat}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    toast("Download started", "success");
  }

  function copyExport() {
    navigator.clipboard.writeText(exportDataText);
    toast("Copied to clipboard", "success");
  }

  function parseImportData() {
    if (!importDataText.trim()) return;

    try {
      if (importFormat === "json") {
        const parsed = JSON.parse(importDataText);
        if (Array.isArray(parsed)) {
          // Extract field names from first record
          const fields = Object.keys(parsed[0]?.data || parsed[0] || {});
          availableFields = fields;
          toast(`Parsed ${parsed.length} records`, "success");
        } else {
          toast("JSON must be an array of records", "error");
        }
      } else {
        // CSV parsing
        const lines = importDataText.trim().split("\n");
        if (lines.length < 2) {
          toast("CSV must have at least a header and one data row", "error");
          return;
        }

        const headers = lines[0].split(",").map((h) => h.replace(/"/g, "").trim());
        availableFields = headers;
        toast(`Parsed CSV with ${headers.length} columns`, "success");
      }
    } catch (error) {
      toast("Failed to parse import data", "error");
      console.error("Error parsing:", error);
    }
  }

  async function performImport() {
    if (!importDataText.trim()) {
      toast("Please provide data to import", "error");
      return;
    }

    loading = true;
    try {
      let records: Array<{ id: string; data: Record<string, unknown> }> = [];

      if (importFormat === "json") {
        const parsed = JSON.parse(importDataText);
        if (Array.isArray(parsed)) {
          records = parsed.map((record, index) => ({
            id: record.id || `imported-${Date.now()}-${index}`,
            data: record.data || record,
          }));
        } else {
          throw new Error("JSON must be an array");
        }
      } else {
        // CSV parsing
        const lines = importDataText.trim().split("\n");
        const headers = lines[0].split(",").map((h) => h.replace(/"/g, "").trim());

        records = lines.slice(1).map((line, index) => {
          const values = line.split(",").map((v) => v.replace(/"/g, "").trim());
          const data: Record<string, unknown> = {};

          headers.forEach((header, i) => {
            const mappedField = importMapping[header] || header;
            data[mappedField] = values[i] || "";
          });

          return {
            id: `imported-${Date.now()}-${index}`,
            data,
          };
        });
      }

      // Import records
      let successCount = 0;
      for (const record of records) {
        try {
          await fetch("/api/put", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify(record),
          });
          successCount++;
        } catch (error) {
          console.error("Error importing record:", record.id, error);
        }
      }

      toast(`Imported ${successCount}/${records.length} records`, "success");
      importDataText = ""; // Clear after successful import
    } catch (error) {
      toast("Import failed", "error");
      console.error("Error importing:", error);
    } finally {
      loading = false;
    }
  }

  function updateMapping(field: string, mappedField: string) {
    importMapping[field] = mappedField;
    importMapping = { ...importMapping };
  }
</script>

<section aria-labelledby="import-export-heading">
  <h3 id="import-export-heading">Import / Export</h3>

  <div class="tab-controls">
    <button
      class="tab-button"
      class:active={activeTab === "export"}
      on:click={() => (activeTab = "export")}
      aria-pressed={activeTab === "export"}
    >
      Export
    </button>
    <button
      class="tab-button"
      class:active={activeTab === "import"}
      on:click={() => (activeTab = "import")}
      aria-pressed={activeTab === "import"}
    >
      Import
    </button>
  </div>

  {#if activeTab === "export"}
    <div class="export-section">
      <div class="export-controls">
        <div class="input-group">
          <label for="export-type">Type to Export</label>
          <input
            id="export-type"
            type="text"
            bind:value={exportType}
            placeholder="Enter type name (e.g., Person, Company)"
          />
        </div>

        <div class="format-group">
          <label for="export-format">Format</label>
          <select id="export-format" bind:value={exportFormat}>
            <option value="json">JSON</option>
            <option value="csv">CSV</option>
          </select>
        </div>

        <button
          on:click={performExport}
          disabled={loading || !exportType.trim()}
          class="export-button"
        >
          {loading ? "Exporting..." : "Export Data"}
        </button>
      </div>

      {#if exportDataText}
        <div class="export-results">
          <div class="export-actions">
            <button on:click={downloadExport} class="secondary"> Download </button>
            <button on:click={copyExport} class="secondary"> Copy to Clipboard </button>
          </div>

          <div class="export-preview">
            <h4>Preview</h4>
            <pre class="export-content">{exportDataText}</pre>
          </div>
        </div>
      {/if}
    </div>
  {:else}
    <div class="import-section">
      <div class="import-controls">
        <div class="format-group">
          <label for="import-format">Format</label>
          <select id="import-format" bind:value={importFormat}>
            <option value="json">JSON</option>
            <option value="csv">CSV</option>
          </select>
        </div>

        <button on:click={parseImportData} disabled={!importDataText.trim()} class="secondary">
          Parse Data
        </button>
      </div>

      <div class="import-data">
        <label for="import-textarea">Data to Import</label>
        <textarea
          id="import-textarea"
          bind:value={importDataText}
          placeholder={importFormat === "json"
            ? "Paste JSON array of records here..."
            : "Paste CSV data here..."}
          rows="8"
        ></textarea>
      </div>

      {#if availableFields.length > 0 && importFormat === "csv"}
        <div class="field-mapping">
          <h4>Field Mapping</h4>
          <p class="mapping-help">Map CSV columns to database fields:</p>
          <div class="mapping-grid">
            {#each availableFields as field}
              <div class="mapping-item">
                <label for="map-{field}">{field}</label>
                <input
                  id="map-{field}"
                  type="text"
                  value={importMapping[field] || field}
                  on:input={(e) => updateMapping(field, e.target.value)}
                  placeholder={field}
                />
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <div class="import-actions">
        <button
          on:click={performImport}
          disabled={loading || !importDataText.trim()}
          class="import-button"
        >
          {loading ? "Importing..." : "Import Data"}
        </button>
      </div>
    </div>
  {/if}
</section>

<style>
  .tab-controls {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1.5rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }

  .tab-button {
    padding: 0.75rem 1.5rem;
    border: none;
    background: transparent;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    font-weight: 500;
  }

  .tab-button:hover {
    background: var(--pico-muted-border-color);
  }

  .tab-button.active {
    border-bottom-color: var(--pico-primary);
    color: var(--pico-primary);
  }

  .export-controls,
  .import-controls {
    display: flex;
    gap: 1rem;
    margin-bottom: 1rem;
    align-items: end;
  }

  .input-group,
  .format-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .input-group {
    flex: 1;
  }

  .export-button,
  .import-button {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 4px;
    font-weight: 500;
  }

  .export-button:hover,
  .import-button:hover {
    opacity: 0.9;
  }

  .export-results {
    margin-top: 1.5rem;
  }

  .export-actions {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .export-preview {
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    padding: 1rem;
  }

  .export-content {
    background: var(--pico-muted-border-color);
    padding: 1rem;
    border-radius: 4px;
    overflow-x: auto;
    font-family: monospace;
    font-size: 0.875rem;
    max-height: 300px;
    overflow-y: auto;
  }

  .import-data {
    margin-bottom: 1.5rem;
  }

  .import-data textarea {
    width: 100%;
    font-family: monospace;
    font-size: 0.875rem;
  }

  .field-mapping {
    margin-bottom: 1.5rem;
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.02);
  }

  .mapping-help {
    color: var(--pico-muted-color);
    font-size: 0.875rem;
    margin-bottom: 1rem;
  }

  .mapping-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.75rem;
  }

  .mapping-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .mapping-item label {
    font-size: 0.875rem;
    font-weight: 500;
  }

  .mapping-item input {
    font-family: monospace;
    font-size: 0.875rem;
  }

  .import-actions {
    display: flex;
    justify-content: flex-end;
  }

  @media (max-width: 768px) {
    .export-controls,
    .import-controls {
      flex-direction: column;
      align-items: stretch;
    }

    .mapping-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
