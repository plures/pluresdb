<script lang="ts">
  import { onMount } from "svelte";
  import { push as toast } from "../lib/toasts";

  let dark = false;
  let activeTab: "subscriptions" | "payments" | "usage" | "analytics" | "invoices" =
    "subscriptions";
  let subscriptions: Array<{
    id: string;
    planName: string;
    status: "active" | "cancelled" | "past_due" | "trialing";
    currentPeriodStart: number;
    currentPeriodEnd: number;
    cancelAtPeriodEnd: boolean;
    trialEnd: number | null;
    price: number;
    currency: string;
    interval: "monthly" | "yearly";
    features: string[];
    limits: {
      nodes: number;
      storage: string;
      users: number;
      apiCalls: number;
    };
  }> = [];
  let paymentMethods: Array<{
    id: string;
    type: "card" | "bank" | "paypal";
    last4: string;
    brand: string;
    expiryMonth: number;
    expiryYear: number;
    isDefault: boolean;
  }> = [];
  let usage: {
    nodes: number;
    storage: string;
    users: number;
    apiCalls: number;
    vectorSearches: number;
    bandwidth: string;
  } = {
    nodes: 0,
    storage: "0 MB",
    users: 0,
    apiCalls: 0,
    vectorSearches: 0,
    bandwidth: "0 MB",
  };
  let invoices: Array<{
    id: string;
    number: string;
    status: "paid" | "pending" | "failed" | "draft";
    amount: number;
    currency: string;
    dueDate: number;
    paidDate: number | null;
    downloadUrl: string;
  }> = [];
  let analytics = {
    monthlyRevenue: 0,
    totalRevenue: 0,
    activeSubscriptions: 0,
    churnRate: 0,
    averageRevenuePerUser: 0,
    growthRate: 0,
  };
  let selectedSubscription: any = null;
  let showPlanDialog = false;
  let showPaymentDialog = false;
  let newPaymentMethod = {
    type: "card" as "card" | "bank" | "paypal",
    cardNumber: "",
    expiryMonth: "",
    expiryYear: "",
    cvv: "",
    name: "",
  };

  // Available plans
  let availablePlans = [
    {
      id: "free",
      name: "Free",
      price: 0,
      currency: "USD",
      interval: "monthly" as const,
      features: ["Up to 1,000 nodes", "1GB storage", "1 user", "Basic support"],
      limits: { nodes: 1000, storage: "1GB", users: 1, apiCalls: 10000 },
    },
    {
      id: "pro",
      name: "Pro",
      price: 29,
      currency: "USD",
      interval: "monthly" as const,
      features: [
        "Up to 100,000 nodes",
        "100GB storage",
        "10 users",
        "Priority support",
        "Advanced analytics",
      ],
      limits: { nodes: 100000, storage: "100GB", users: 10, apiCalls: 1000000 },
    },
    {
      id: "enterprise",
      name: "Enterprise",
      price: 99,
      currency: "USD",
      interval: "monthly" as const,
      features: [
        "Unlimited nodes",
        "1TB storage",
        "Unlimited users",
        "24/7 support",
        "Custom integrations",
      ],
      limits: { nodes: -1, storage: "1TB", users: -1, apiCalls: -1 },
    },
  ];

  $: dark = document.documentElement.getAttribute("data-theme") === "dark";

  onMount(() => {
    loadBillingData();
  });

  async function loadBillingData() {
    try {
      // Simulate billing data
      subscriptions = [
        {
          id: "sub-1",
          planName: "Pro",
          status: "active",
          currentPeriodStart: Date.now() - 2592000000, // 30 days ago
          currentPeriodEnd: Date.now() + 2592000000, // 30 days from now
          cancelAtPeriodEnd: false,
          trialEnd: null,
          price: 29,
          currency: "USD",
          interval: "monthly",
          features: ["Up to 100,000 nodes", "100GB storage", "10 users", "Priority support"],
          limits: { nodes: 100000, storage: "100GB", users: 10, apiCalls: 1000000 },
        },
      ];

      paymentMethods = [
        {
          id: "pm-1",
          type: "card",
          last4: "4242",
          brand: "Visa",
          expiryMonth: 12,
          expiryYear: 2025,
          isDefault: true,
        },
      ];

      usage = {
        nodes: 25000,
        storage: "25GB",
        users: 5,
        apiCalls: 250000,
        vectorSearches: 15000,
        bandwidth: "500MB",
      };

      invoices = [
        {
          id: "inv-1",
          number: "INV-2024-001",
          status: "paid",
          amount: 29,
          currency: "USD",
          dueDate: Date.now() - 86400000, // 1 day ago
          paidDate: Date.now() - 86400000,
          downloadUrl: "/api/invoices/inv-1/download",
        },
        {
          id: "inv-2",
          number: "INV-2024-002",
          status: "pending",
          amount: 29,
          currency: "USD",
          dueDate: Date.now() + 2592000000, // 30 days from now
          paidDate: null,
          downloadUrl: "/api/invoices/inv-2/download",
        },
      ];

      analytics = {
        monthlyRevenue: 2900,
        totalRevenue: 34800,
        activeSubscriptions: 100,
        churnRate: 2.5,
        averageRevenuePerUser: 29,
        growthRate: 15.2,
      };
    } catch (error) {
      toast("Failed to load billing data", "error");
      console.error("Error loading billing data:", error);
    }
  }

  function selectSubscription(subscription: any) {
    selectedSubscription = subscription;
  }

  async function cancelSubscription(subscriptionId: string) {
    if (confirm("Are you sure you want to cancel this subscription?")) {
      try {
        const subscription = subscriptions.find((s) => s.id === subscriptionId);
        if (subscription) {
          subscription.cancelAtPeriodEnd = true;
          toast("Subscription will be cancelled at the end of the current period", "success");
        }
      } catch (error) {
        toast("Failed to cancel subscription", "error");
        console.error("Cancel subscription error:", error);
      }
    }
  }

  async function reactivateSubscription(subscriptionId: string) {
    try {
      const subscription = subscriptions.find((s) => s.id === subscriptionId);
      if (subscription) {
        subscription.cancelAtPeriodEnd = false;
        toast("Subscription reactivated", "success");
      }
    } catch (error) {
      toast("Failed to reactivate subscription", "error");
      console.error("Reactivate subscription error:", error);
    }
  }

  async function changePlan(subscriptionId: string, newPlanId: string) {
    try {
      const subscription = subscriptions.find((s) => s.id === subscriptionId);
      const newPlan = availablePlans.find((p) => p.id === newPlanId);
      if (subscription && newPlan) {
        subscription.planName = newPlan.name;
        subscription.price = newPlan.price;
        subscription.features = newPlan.features;
        subscription.limits = newPlan.limits;
        toast("Plan changed successfully", "success");
      }
    } catch (error) {
      toast("Failed to change plan", "error");
      console.error("Change plan error:", error);
    }
  }

  async function addPaymentMethod() {
    if (!newPaymentMethod.cardNumber || !newPaymentMethod.name) {
      toast("Please fill in all required fields", "error");
      return;
    }

    try {
      const paymentMethod = {
        id: `pm-${Date.now()}`,
        type: newPaymentMethod.type,
        last4: newPaymentMethod.cardNumber.slice(-4),
        brand: "Visa", // In real app, detect from card number
        expiryMonth: parseInt(newPaymentMethod.expiryMonth),
        expiryYear: parseInt(newPaymentMethod.expiryYear),
        isDefault: paymentMethods.length === 0,
      };

      paymentMethods = [...paymentMethods, paymentMethod];
      newPaymentMethod = {
        type: "card",
        cardNumber: "",
        expiryMonth: "",
        expiryYear: "",
        cvv: "",
        name: "",
      };
      showPaymentDialog = false;
      toast("Payment method added successfully", "success");
    } catch (error) {
      toast("Failed to add payment method", "error");
      console.error("Add payment method error:", error);
    }
  }

  async function removePaymentMethod(paymentMethodId: string) {
    if (confirm("Are you sure you want to remove this payment method?")) {
      try {
        paymentMethods = paymentMethods.filter((pm) => pm.id !== paymentMethodId);
        toast("Payment method removed", "success");
      } catch (error) {
        toast("Failed to remove payment method", "error");
        console.error("Remove payment method error:", error);
      }
    }
  }

  async function downloadInvoice(invoiceId: string) {
    try {
      // Simulate invoice download
      const invoice = invoices.find((i) => i.id === invoiceId);
      if (invoice) {
        window.open(invoice.downloadUrl, "_blank");
        toast("Invoice download started", "success");
      }
    } catch (error) {
      toast("Failed to download invoice", "error");
      console.error("Download invoice error:", error);
    }
  }

  function formatCurrency(amount: number, currency: string): string {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: currency,
    }).format(amount);
  }

  function formatTimestamp(timestamp: number): string {
    return new Date(timestamp).toLocaleDateString();
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case "active":
        return "var(--success-color)";
      case "cancelled":
        return "var(--error-color)";
      case "past_due":
        return "var(--warning-color)";
      case "trialing":
        return "var(--primary-color)";
      case "paid":
        return "var(--success-color)";
      case "pending":
        return "var(--warning-color)";
      case "failed":
        return "var(--error-color)";
      case "draft":
        return "var(--muted-color)";
      default:
        return "var(--muted-color)";
    }
  }

  function getStatusIcon(status: string): string {
    switch (status) {
      case "active":
        return "✅";
      case "cancelled":
        return "❌";
      case "past_due":
        return "⚠️";
      case "trialing":
        return "🆓";
      case "paid":
        return "✅";
      case "pending":
        return "⏳";
      case "failed":
        return "❌";
      case "draft":
        return "📝";
      default:
        return "⚪";
    }
  }

  function getUsagePercentage(current: number, limit: number): number {
    if (limit === -1) return 0; // Unlimited
    return Math.min((current / limit) * 100, 100);
  }

  function formatBytes(bytes: string): string {
    return bytes; // Already formatted in the data
  }
</script>

<section aria-labelledby="billing-panel-heading">
  <h3 id="billing-panel-heading">Billing & Payments</h3>

  <div class="billing-layout">
    <!-- Tabs -->
    <div class="billing-tabs">
      <button
        class="tab-button"
        class:active={activeTab === "subscriptions"}
        on:click={() => (activeTab = "subscriptions")}
      >
        Subscriptions ({subscriptions.length})
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "payments"}
        on:click={() => (activeTab = "payments")}
      >
        Payment Methods ({paymentMethods.length})
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "usage"}
        on:click={() => (activeTab = "usage")}
      >
        Usage
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "invoices"}
        on:click={() => (activeTab = "invoices")}
      >
        Invoices ({invoices.length})
      </button>
      <button
        class="tab-button"
        class:active={activeTab === "analytics"}
        on:click={() => (activeTab = "analytics")}
      >
        Analytics
      </button>
    </div>

    <!-- Content -->
    <div class="billing-content">
      {#if activeTab === "subscriptions"}
        <div class="subscriptions-section">
          <div class="section-header">
            <h4>Subscription Management</h4>
            <button on:click={() => (showPlanDialog = true)} class="primary"> Change Plan </button>
          </div>

          <div class="subscriptions-list">
            {#each subscriptions as subscription}
              <div
                class="subscription-item"
                class:selected={selectedSubscription?.id === subscription.id}
                role="button"
                tabindex="0"
                on:click={() => selectSubscription(subscription)}
                on:keydown={(e) => e.key === "Enter" && selectSubscription(subscription)}
              >
                <div class="subscription-header">
                  <div class="subscription-info">
                    <span class="subscription-plan">{subscription.planName}</span>
                    <span class="subscription-price">
                      {formatCurrency(
                        subscription.price,
                        subscription.currency,
                      )}/{subscription.interval}
                    </span>
                  </div>
                  <div
                    class="subscription-status"
                    style="color: {getStatusColor(subscription.status)}"
                  >
                    {getStatusIcon(subscription.status)}
                    {subscription.status}
                  </div>
                </div>
                <div class="subscription-details">
                  <div class="subscription-period">
                    <span
                      >Current period: {formatTimestamp(subscription.currentPeriodStart)} - {formatTimestamp(
                        subscription.currentPeriodEnd,
                      )}</span
                    >
                    {#if subscription.cancelAtPeriodEnd}
                      <span class="cancellation-notice">⚠️ Cancels at period end</span>
                    {/if}
                  </div>
                  <div class="subscription-features">
                    {#each subscription.features as feature}
                      <span class="feature-tag">{feature}</span>
                    {/each}
                  </div>
                </div>
                <div class="subscription-actions">
                  {#if subscription.cancelAtPeriodEnd}
                    <button
                      on:click|stopPropagation={() => reactivateSubscription(subscription.id)}
                      class="small"
                    >
                      Reactivate
                    </button>
                  {:else}
                    <button
                      on:click|stopPropagation={() => cancelSubscription(subscription.id)}
                      class="small"
                    >
                      Cancel
                    </button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === "payments"}
        <div class="payments-section">
          <div class="section-header">
            <h4>Payment Methods</h4>
            <button on:click={() => (showPaymentDialog = true)} class="primary">
              Add Payment Method
            </button>
          </div>

          <div class="payment-methods-list">
            {#each paymentMethods as paymentMethod}
              <div class="payment-method-item">
                <div class="payment-method-info">
                  <div class="payment-method-details">
                    <span class="payment-method-type">
                      {paymentMethod.type === "card"
                        ? "💳"
                        : paymentMethod.type === "bank"
                          ? "🏦"
                          : "💳"}
                      {paymentMethod.brand} •••• {paymentMethod.last4}
                    </span>
                    <span class="payment-method-expiry">
                      Expires {paymentMethod.expiryMonth}/{paymentMethod.expiryYear}
                    </span>
                  </div>
                  <div class="payment-method-status">
                    {#if paymentMethod.isDefault}
                      <span class="default-badge">Default</span>
                    {/if}
                  </div>
                </div>
                <div class="payment-method-actions">
                  <button on:click={() => removePaymentMethod(paymentMethod.id)} class="small">
                    Remove
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === "usage"}
        <div class="usage-section">
          <h4>Usage & Limits</h4>

          <div class="usage-grid">
            <div class="usage-card">
              <div class="usage-header">
                <span class="usage-title">Nodes</span>
                <span class="usage-value">{usage.nodes.toLocaleString()}</span>
              </div>
              <div class="usage-bar">
                <div
                  class="usage-fill"
                  style="width: {getUsagePercentage(
                    usage.nodes,
                    selectedSubscription?.limits?.nodes || 1000,
                  )}%"
                ></div>
              </div>
              <div class="usage-limit">
                Limit: {selectedSubscription?.limits?.nodes === -1
                  ? "Unlimited"
                  : selectedSubscription?.limits?.nodes?.toLocaleString() || "1,000"}
              </div>
            </div>

            <div class="usage-card">
              <div class="usage-header">
                <span class="usage-title">Storage</span>
                <span class="usage-value">{usage.storage}</span>
              </div>
              <div class="usage-bar">
                <div
                  class="usage-fill"
                  style="width: {getUsagePercentage(
                    parseInt(usage.storage),
                    parseInt(selectedSubscription?.limits?.storage || '1GB'),
                  )}%"
                ></div>
              </div>
              <div class="usage-limit">
                Limit: {selectedSubscription?.limits?.storage || "1GB"}
              </div>
            </div>

            <div class="usage-card">
              <div class="usage-header">
                <span class="usage-title">Users</span>
                <span class="usage-value">{usage.users}</span>
              </div>
              <div class="usage-bar">
                <div
                  class="usage-fill"
                  style="width: {getUsagePercentage(
                    usage.users,
                    selectedSubscription?.limits?.users || 1,
                  )}%"
                ></div>
              </div>
              <div class="usage-limit">
                Limit: {selectedSubscription?.limits?.users === -1
                  ? "Unlimited"
                  : selectedSubscription?.limits?.users || 1}
              </div>
            </div>

            <div class="usage-card">
              <div class="usage-header">
                <span class="usage-title">API Calls</span>
                <span class="usage-value">{usage.apiCalls.toLocaleString()}</span>
              </div>
              <div class="usage-bar">
                <div
                  class="usage-fill"
                  style="width: {getUsagePercentage(
                    usage.apiCalls,
                    selectedSubscription?.limits?.apiCalls || 10000,
                  )}%"
                ></div>
              </div>
              <div class="usage-limit">
                Limit: {selectedSubscription?.limits?.apiCalls === -1
                  ? "Unlimited"
                  : selectedSubscription?.limits?.apiCalls?.toLocaleString() || "10,000"}
              </div>
            </div>
          </div>
        </div>
      {:else if activeTab === "invoices"}
        <div class="invoices-section">
          <h4>Invoices</h4>

          <div class="invoices-list">
            {#each invoices as invoice}
              <div class="invoice-item">
                <div class="invoice-header">
                  <div class="invoice-info">
                    <span class="invoice-number">{invoice.number}</span>
                    <span class="invoice-amount"
                      >{formatCurrency(invoice.amount, invoice.currency)}</span
                    >
                  </div>
                  <div class="invoice-status" style="color: {getStatusColor(invoice.status)}">
                    {getStatusIcon(invoice.status)}
                    {invoice.status}
                  </div>
                </div>
                <div class="invoice-details">
                  <span>Due: {formatTimestamp(invoice.dueDate)}</span>
                  {#if invoice.paidDate}
                    <span>Paid: {formatTimestamp(invoice.paidDate)}</span>
                  {/if}
                </div>
                <div class="invoice-actions">
                  <button on:click={() => downloadInvoice(invoice.id)} class="small">
                    Download
                  </button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if activeTab === "analytics"}
        <div class="analytics-section">
          <h4>Billing Analytics</h4>

          <div class="analytics-grid">
            <div class="analytics-card">
              <div class="analytics-title">Monthly Revenue</div>
              <div class="analytics-value">{formatCurrency(analytics.monthlyRevenue, "USD")}</div>
            </div>

            <div class="analytics-card">
              <div class="analytics-title">Total Revenue</div>
              <div class="analytics-value">{formatCurrency(analytics.totalRevenue, "USD")}</div>
            </div>

            <div class="analytics-card">
              <div class="analytics-title">Active Subscriptions</div>
              <div class="analytics-value">{analytics.activeSubscriptions}</div>
            </div>

            <div class="analytics-card">
              <div class="analytics-title">Churn Rate</div>
              <div class="analytics-value">{analytics.churnRate}%</div>
            </div>

            <div class="analytics-card">
              <div class="analytics-title">ARPU</div>
              <div class="analytics-value">
                {formatCurrency(analytics.averageRevenuePerUser, "USD")}
              </div>
            </div>

            <div class="analytics-card">
              <div class="analytics-title">Growth Rate</div>
              <div class="analytics-value">{analytics.growthRate}%</div>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>

  <!-- Plan Selection Dialog -->
  {#if showPlanDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Change Plan</h4>
        <div class="plans-grid">
          {#each availablePlans as plan}
            <div class="plan-card" class:selected={selectedSubscription?.planName === plan.name}>
              <div class="plan-header">
                <span class="plan-name">{plan.name}</span>
                <span class="plan-price">
                  {formatCurrency(plan.price, plan.currency)}/{plan.interval}
                </span>
              </div>
              <div class="plan-features">
                {#each plan.features as feature}
                  <span class="plan-feature">{feature}</span>
                {/each}
              </div>
              <button
                on:click={() => changePlan(selectedSubscription?.id, plan.id)}
                class="plan-select"
                disabled={selectedSubscription?.planName === plan.name}
              >
                {selectedSubscription?.planName === plan.name ? "Current Plan" : "Select Plan"}
              </button>
            </div>
          {/each}
        </div>
        <div class="dialog-actions">
          <button on:click={() => (showPlanDialog = false)} class="secondary"> Cancel </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Payment Method Dialog -->
  {#if showPaymentDialog}
    <div class="dialog-overlay">
      <div class="dialog">
        <h4>Add Payment Method</h4>
        <select bind:value={newPaymentMethod.type}>
          <option value="card">Credit Card</option>
          <option value="bank">Bank Account</option>
          <option value="paypal">PayPal</option>
        </select>
        <input type="text" bind:value={newPaymentMethod.cardNumber} placeholder="Card Number" />
        <input type="text" bind:value={newPaymentMethod.name} placeholder="Cardholder Name" />
        <div class="form-row">
          <input
            type="text"
            bind:value={newPaymentMethod.expiryMonth}
            placeholder="MM"
            maxlength="2"
          />
          <input
            type="text"
            bind:value={newPaymentMethod.expiryYear}
            placeholder="YYYY"
            maxlength="4"
          />
          <input type="text" bind:value={newPaymentMethod.cvv} placeholder="CVV" maxlength="4" />
        </div>
        <div class="dialog-actions">
          <button
            on:click={addPaymentMethod}
            disabled={!newPaymentMethod.cardNumber.trim() || !newPaymentMethod.name.trim()}
          >
            Add Payment Method
          </button>
          <button on:click={() => (showPaymentDialog = false)} class="secondary"> Cancel </button>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .billing-layout {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .billing-tabs {
    display: flex;
    gap: 0.5rem;
    border-bottom: 1px solid var(--pico-muted-border-color);
  }

  .tab-button {
    padding: 0.75rem 1rem;
    border: none;
    background: transparent;
    color: var(--pico-muted-color);
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
  }

  .tab-button:hover {
    color: var(--pico-primary);
  }

  .tab-button.active {
    color: var(--pico-primary);
    border-bottom-color: var(--pico-primary);
  }

  .billing-content {
    flex: 1;
    padding: 1rem;
    background: var(--pico-background-color);
    border-radius: 8px;
    border: 1px solid var(--pico-muted-border-color);
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .subscriptions-list,
  .payment-methods-list,
  .invoices-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .subscription-item,
  .payment-method-item,
  .invoice-item {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    cursor: pointer;
    transition: all 0.2s;
  }

  .subscription-item:hover,
  .payment-method-item:hover,
  .invoice-item:hover {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .subscription-item.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .subscription-header,
  .payment-method-info,
  .invoice-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .subscription-info,
  .payment-method-details,
  .invoice-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .subscription-plan,
  .payment-method-type,
  .invoice-number {
    font-weight: 600;
    font-size: 1rem;
  }

  .subscription-price,
  .invoice-amount {
    font-size: 0.875rem;
    color: var(--pico-primary);
  }

  .subscription-details,
  .invoice-details {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.875rem;
    margin-bottom: 0.5rem;
  }

  .subscription-features {
    display: flex;
    gap: 0.25rem;
    flex-wrap: wrap;
  }

  .feature-tag {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  .cancellation-notice {
    color: var(--warning-color);
    font-weight: 600;
  }

  .subscription-actions,
  .payment-method-actions,
  .invoice-actions {
    display: flex;
    gap: 0.5rem;
  }

  .usage-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
  }

  .usage-card {
    padding: 1rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
  }

  .usage-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .usage-title {
    font-weight: 600;
  }

  .usage-value {
    font-size: 1.25rem;
    font-weight: bold;
    color: var(--pico-primary);
  }

  .usage-bar {
    width: 100%;
    height: 8px;
    background: var(--pico-muted-border-color);
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 0.5rem;
  }

  .usage-fill {
    height: 100%;
    background: var(--pico-primary);
    transition: width 0.3s ease;
  }

  .usage-limit {
    font-size: 0.75rem;
    color: var(--pico-muted-color);
  }

  .analytics-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
  }

  .analytics-card {
    padding: 1.5rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    text-align: center;
  }

  .analytics-title {
    font-size: 0.875rem;
    color: var(--pico-muted-color);
    margin-bottom: 0.5rem;
  }

  .analytics-value {
    font-size: 2rem;
    font-weight: bold;
    color: var(--pico-primary);
  }

  .plans-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .plan-card {
    padding: 1.5rem;
    border: 1px solid var(--pico-muted-border-color);
    border-radius: 8px;
    background: var(--pico-muted-border-color);
    cursor: pointer;
    transition: all 0.2s;
  }

  .plan-card:hover {
    border-color: var(--pico-primary);
  }

  .plan-card.selected {
    border-color: var(--pico-primary);
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
  }

  .plan-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .plan-name {
    font-weight: 600;
    font-size: 1.25rem;
  }

  .plan-price {
    font-size: 1.5rem;
    font-weight: bold;
    color: var(--pico-primary);
  }

  .plan-features {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .plan-feature {
    font-size: 0.875rem;
  }

  .plan-select {
    width: 100%;
    padding: 0.75rem;
    border: none;
    border-radius: 4px;
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
    cursor: pointer;
    font-weight: 600;
  }

  .plan-select:disabled {
    background: var(--pico-muted-color);
    cursor: not-allowed;
  }

  .default-badge {
    background: var(--success-color);
    color: white;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  .dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .dialog {
    background: var(--pico-background-color);
    padding: 2rem;
    border-radius: 8px;
    min-width: 600px;
    max-width: 800px;
  }

  .dialog h4 {
    margin-bottom: 1rem;
  }

  .dialog input,
  .dialog select {
    width: 100%;
    margin-bottom: 1rem;
  }

  .form-row {
    display: flex;
    gap: 0.5rem;
  }

  .form-row input {
    flex: 1;
  }

  .dialog-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  .small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }

  @media (max-width: 768px) {
    .billing-tabs {
      flex-wrap: wrap;
    }

    .tab-button {
      flex: 1;
      min-width: 120px;
    }

    .usage-grid,
    .analytics-grid,
    .plans-grid {
      grid-template-columns: 1fr;
    }

    .form-row {
      flex-direction: column;
    }
  }
</style>
