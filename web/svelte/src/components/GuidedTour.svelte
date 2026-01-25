<script lang="ts">
  import { onMount } from "svelte";
  
  export let onViewChange: (view: string) => void;
  
  let tourActive = false;
  let currentStep = 0;
  let tourDismissed = false;
  
  const tourSteps = [
    {
      title: "Welcome to PluresDB!",
      content: "PluresDB is a local-first, P2P graph database with SQLite compatibility. Let's take a quick tour of the main features.",
      view: "data",
      highlight: null
    },
    {
      title: "Data Explorer",
      content: "View and manage your nodes here. Use the virtualized list for efficient browsing of large datasets.",
      view: "data",
      highlight: null
    },
    {
      title: "Type Management",
      content: "Define schemas for your data types with JSON Schema validation. Track type statistics and manage required fields.",
      view: "types",
      highlight: null
    },
    {
      title: "History & Time Travel",
      content: "View version history of any node, see diffs between versions, and restore previous states.",
      view: "history",
      highlight: null
    },
    {
      title: "Graph Visualization",
      content: "Explore your data as an interactive graph. Filter by type, search nodes, and use different layout algorithms.",
      view: "graph",
      highlight: null
    },
    {
      title: "Vector Search",
      content: "Perform similarity search using embeddings. Toggle between brute-force and HNSW algorithms for performance.",
      view: "vector",
      highlight: null
    },
    {
      title: "Query Builder",
      content: "Build complex queries visually with AND/OR conditions, save queries, and explore raw DSL mode.",
      view: "queries",
      highlight: null
    },
    {
      title: "Rules & Automation",
      content: "Create rules with conditions and actions. Automatically set properties or create relations when conditions match.",
      view: "rules",
      highlight: null
    },
    {
      title: "P2P Mesh Network",
      content: "Connect with peers, monitor bandwidth and message rates, and control synchronization.",
      view: "mesh",
      highlight: null
    },
    {
      title: "Tour Complete!",
      content: "You've seen the main features. Explore on your own, or load example datasets from Import/Export to get started!",
      view: "data",
      highlight: null
    }
  ];
  
  onMount(() => {
    try {
      const dismissed = localStorage.getItem("pluresdb_tour_dismissed");
      if (!dismissed) {
        const firstVisit = !localStorage.getItem("pluresdb_visited");
        if (firstVisit) {
          localStorage.setItem("pluresdb_visited", "true");
          setTimeout(() => {
            tourActive = true;
          }, 1000);
        }
      } else {
        tourDismissed = true;
      }
    } catch (error) {
      // localStorage may not be available (private mode, quota exceeded, etc.)
      console.warn("Tour: localStorage not available", error);
      // Gracefully degrade - don't show tour if storage unavailable
      tourDismissed = true;
    }
  });
  
  function startTour() {
    currentStep = 0;
    tourActive = true;
    try {
      localStorage.removeItem("pluresdb_tour_dismissed");
      tourDismissed = false;
    } catch (error) {
      console.warn("Tour: Failed to update localStorage", error);
    }
    if (tourSteps[0].view) {
      onViewChange(tourSteps[0].view);
    }
  }
  
  function nextStep() {
    if (currentStep < tourSteps.length - 1) {
      currentStep++;
      if (tourSteps[currentStep].view) {
        onViewChange(tourSteps[currentStep].view);
      }
    } else {
      endTour();
    }
  }
  
  function prevStep() {
    if (currentStep > 0) {
      currentStep--;
      if (tourSteps[currentStep].view) {
        onViewChange(tourSteps[currentStep].view);
      }
    }
  }
  
  function endTour() {
    tourActive = false;
    try {
      localStorage.setItem("pluresdb_tour_dismissed", "true");
      tourDismissed = true;
    } catch (error) {
      console.warn("Tour: Failed to update localStorage", error);
      // Still set the flag even if we can't persist it
      tourDismissed = true;
    }
  }
  
  function skipTour() {
    endTour();
  }
</script>

{#if !tourDismissed && !tourActive}
  <div class="tour-prompt" role="dialog" aria-labelledby="tour-prompt-title">
    <p id="tour-prompt-title"><strong>New to PluresDB?</strong></p>
    <p>Take a quick tour to learn the main features</p>
    <div class="button-group">
      <button on:click={startTour} aria-label="Start guided tour">Start Tour</button>
      <button class="secondary" on:click={skipTour} aria-label="Skip tour">Skip</button>
    </div>
  </div>
{/if}

{#if tourActive}
  <div class="tour-overlay" role="dialog" aria-labelledby="tour-step-title" aria-describedby="tour-step-content">
    <div class="tour-card">
      <h3 id="tour-step-title">{tourSteps[currentStep].title}</h3>
      <p id="tour-step-content">{tourSteps[currentStep].content}</p>
      <div class="tour-progress">
        Step {currentStep + 1} of {tourSteps.length}
      </div>
      <div class="tour-buttons">
        {#if currentStep > 0}
          <button class="secondary" on:click={prevStep} aria-label="Previous step">Previous</button>
        {/if}
        <button on:click={nextStep} aria-label={currentStep < tourSteps.length - 1 ? "Next step" : "Finish tour"}>
          {currentStep < tourSteps.length - 1 ? "Next" : "Finish"}
        </button>
        <button class="secondary" on:click={skipTour} aria-label="Skip tour">Skip Tour</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .tour-prompt {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: var(--pico-card-background-color);
    padding: 1rem;
    border-radius: 8px;
    box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
    max-width: 300px;
    z-index: 1000;
    border: 2px solid var(--pico-primary);
  }
  
  .tour-prompt p {
    margin-bottom: 0.5rem;
  }
  
  .button-group {
    display: flex;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  
  .button-group button {
    flex: 1;
    margin: 0;
  }
  
  .tour-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
  }
  
  .tour-card {
    background: var(--pico-card-background-color);
    padding: 2rem;
    border-radius: 8px;
    max-width: 500px;
    box-shadow: 0 8px 16px rgba(0, 0, 0, 0.2);
  }
  
  .tour-card h3 {
    margin-top: 0;
    color: var(--pico-primary);
  }
  
  .tour-progress {
    text-align: center;
    color: var(--pico-muted-color);
    margin: 1rem 0;
    font-size: 0.9rem;
  }
  
  .tour-buttons {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }
  
  .tour-buttons button {
    margin: 0;
  }
</style>
