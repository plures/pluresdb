// Rusty Gun Subscription Management System
// Handles customer subscriptions, billing, and plan management

export interface SubscriptionPlan {
  id: string;
  name: string;
  description: string;
  price: number;
  currency: string;
  billingCycle: 'monthly' | 'yearly';
  features: string[];
  limits: {
    instances: number;
    storage: number; // in GB
    users: number;
    apiCalls: number; // per month
    support: 'community' | 'email' | 'phone' | 'dedicated';
  };
  azureResources: {
    appServicePlan: string;
    postgresqlSku: string;
    redisSku: string;
    storageSku: string;
  };
}

export interface CustomerSubscription {
  id: string;
  customerId: string;
  planId: string;
  status: 'active' | 'cancelled' | 'past_due' | 'trialing';
  currentPeriodStart: Date;
  currentPeriodEnd: Date;
  trialEnd?: Date;
  cancelAtPeriodEnd: boolean;
  azureResourceGroup: string;
  azureResources: {
    appServiceName: string;
    postgresqlServerName: string;
    redisCacheName: string;
    storageAccountName: string;
  };
  usage: {
    instances: number;
    storageUsed: number; // in GB
    apiCalls: number; // current month
    lastUpdated: Date;
  };
  billing: {
    stripeSubscriptionId?: string;
    stripeCustomerId?: string;
    paymentMethodId?: string;
    nextBillingDate: Date;
    amount: number;
    currency: string;
  };
}

export class SubscriptionManager {
  private plans: Map<string, SubscriptionPlan> = new Map();
  private subscriptions: Map<string, CustomerSubscription> = new Map();

  constructor() {
    this.initializePlans();
  }

  private initializePlans(): void {
    const plans: SubscriptionPlan[] = [
      {
        id: 'developer',
        name: 'Developer',
        description: 'Perfect for individual developers and small projects',
        price: 29,
        currency: 'USD',
        billingCycle: 'monthly',
        features: [
          '1 Rusty Gun instance',
          '1GB storage',
          'Community support',
          'Basic monitoring',
          'Standard documentation'
        ],
        limits: {
          instances: 1,
          storage: 1,
          users: 1,
          apiCalls: 10000,
          support: 'community'
        },
        azureResources: {
          appServicePlan: 'B1',
          postgresqlSku: 'B_Gen5_1',
          redisSku: 'Basic_C0',
          storageSku: 'Standard_LRS'
        }
      },
      {
        id: 'team',
        name: 'Team',
        description: 'Ideal for small teams and growing projects',
        price: 99,
        currency: 'USD',
        billingCycle: 'monthly',
        features: [
          '5 Rusty Gun instances',
          '10GB storage',
          'Email support',
          'Advanced monitoring',
          'Priority documentation',
          'Team collaboration tools'
        ],
        limits: {
          instances: 5,
          storage: 10,
          users: 10,
          apiCalls: 100000,
          support: 'email'
        },
        azureResources: {
          appServicePlan: 'S1',
          postgresqlSku: 'GP_Gen5_2',
          redisSku: 'Standard_C1',
          storageSku: 'Standard_LRS'
        }
      },
      {
        id: 'business',
        name: 'Business',
        description: 'Perfect for medium-sized businesses',
        price: 299,
        currency: 'USD',
        billingCycle: 'monthly',
        features: [
          '20 Rusty Gun instances',
          '100GB storage',
          'Phone support',
          'Advanced monitoring & alerting',
          'Custom integrations',
          'SLA guarantee',
          'Backup & recovery'
        ],
        limits: {
          instances: 20,
          storage: 100,
          users: 50,
          apiCalls: 1000000,
          support: 'phone'
        },
        azureResources: {
          appServicePlan: 'S2',
          postgresqlSku: 'GP_Gen5_4',
          redisSku: 'Standard_C2',
          storageSku: 'Standard_GRS'
        }
      },
      {
        id: 'enterprise',
        name: 'Enterprise',
        description: 'For large organizations with demanding requirements',
        price: 999,
        currency: 'USD',
        billingCycle: 'monthly',
        features: [
          'Unlimited instances',
          '1TB storage',
          'Dedicated support engineer',
          '24/7 phone support',
          'Custom monitoring & alerting',
          'White-label options',
          'Custom integrations',
          '99.9% SLA guarantee',
          'Advanced security features',
          'Compliance support'
        ],
        limits: {
          instances: -1, // unlimited
          storage: 1000,
          users: -1, // unlimited
          apiCalls: -1, // unlimited
          support: 'dedicated'
        },
        azureResources: {
          appServicePlan: 'P1V2',
          postgresqlSku: 'GP_Gen5_8',
          redisSku: 'Premium_P1',
          storageSku: 'Premium_LRS'
        }
      }
    ];

    plans.forEach(plan => {
      this.plans.set(plan.id, plan);
    });
  }

  // Get all available plans
  getPlans(): SubscriptionPlan[] {
    return Array.from(this.plans.values());
  }

  // Get a specific plan
  getPlan(planId: string): SubscriptionPlan | undefined {
    return this.plans.get(planId);
  }

  // Create a new subscription
  async createSubscription(
    customerId: string,
    planId: string,
    paymentMethodId?: string
  ): Promise<CustomerSubscription> {
    const plan = this.getPlan(planId);
    if (!plan) {
      throw new Error(`Plan ${planId} not found`);
    }

    const subscriptionId = `sub_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const now = new Date();
    const periodEnd = new Date(now);
    periodEnd.setMonth(periodEnd.getMonth() + 1);

    const subscription: CustomerSubscription = {
      id: subscriptionId,
      customerId,
      planId,
      status: 'trialing',
      currentPeriodStart: now,
      currentPeriodEnd: periodEnd,
      trialEnd: new Date(now.getTime() + 30 * 24 * 60 * 60 * 1000), // 30 days
      cancelAtPeriodEnd: false,
      azureResourceGroup: `rusty-gun-${customerId}-rg`,
      azureResources: {
        appServiceName: `rusty-gun-${customerId}-app`,
        postgresqlServerName: `rusty-gun-${customerId}-db`,
        redisCacheName: `rusty-gun-${customerId}-redis`,
        storageAccountName: `rustygun${customerId}storage`
      },
      usage: {
        instances: 0,
        storageUsed: 0,
        apiCalls: 0,
        lastUpdated: now
      },
      billing: {
        paymentMethodId,
        nextBillingDate: periodEnd,
        amount: plan.price,
        currency: plan.currency
      }
    };

    this.subscriptions.set(subscriptionId, subscription);
    return subscription;
  }

  // Get subscription by ID
  getSubscription(subscriptionId: string): CustomerSubscription | undefined {
    return this.subscriptions.get(subscriptionId);
  }

  // Get subscriptions for a customer
  getCustomerSubscriptions(customerId: string): CustomerSubscription[] {
    return Array.from(this.subscriptions.values())
      .filter(sub => sub.customerId === customerId);
  }

  // Update subscription status
  async updateSubscriptionStatus(
    subscriptionId: string,
    status: CustomerSubscription['status']
  ): Promise<void> {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      throw new Error(`Subscription ${subscriptionId} not found`);
    }

    subscription.status = status;
    this.subscriptions.set(subscriptionId, subscription);
  }

  // Cancel subscription
  async cancelSubscription(
    subscriptionId: string,
    cancelAtPeriodEnd: boolean = true
  ): Promise<void> {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      throw new Error(`Subscription ${subscriptionId} not found`);
    }

    if (cancelAtPeriodEnd) {
      subscription.cancelAtPeriodEnd = true;
    } else {
      subscription.status = 'cancelled';
    }

    this.subscriptions.set(subscriptionId, subscription);
  }

  // Update usage metrics
  async updateUsage(
    subscriptionId: string,
    usage: Partial<CustomerSubscription['usage']>
  ): Promise<void> {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      throw new Error(`Subscription ${subscriptionId} not found`);
    }

    subscription.usage = {
      ...subscription.usage,
      ...usage,
      lastUpdated: new Date()
    };

    this.subscriptions.set(subscriptionId, subscription);
  }

  // Check if usage exceeds limits
  checkUsageLimits(subscriptionId: string): {
    withinLimits: boolean;
    exceededLimits: string[];
  } {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      throw new Error(`Subscription ${subscriptionId} not found`);
    }

    const plan = this.getPlan(subscription.planId);
    if (!plan) {
      throw new Error(`Plan ${subscription.planId} not found`);
    }

    const exceededLimits: string[] = [];

    // Check instance limit
    if (plan.limits.instances !== -1 && subscription.usage.instances > plan.limits.instances) {
      exceededLimits.push('instances');
    }

    // Check storage limit
    if (subscription.usage.storageUsed > plan.limits.storage) {
      exceededLimits.push('storage');
    }

    // Check API calls limit
    if (plan.limits.apiCalls !== -1 && subscription.usage.apiCalls > plan.limits.apiCalls) {
      exceededLimits.push('apiCalls');
    }

    return {
      withinLimits: exceededLimits.length === 0,
      exceededLimits
    };
  }

  // Get billing information
  getBillingInfo(subscriptionId: string): {
    currentPeriod: { start: Date; end: Date };
    nextBillingDate: Date;
    amount: number;
    currency: string;
    status: string;
  } {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      throw new Error(`Subscription ${subscriptionId} not found`);
    }

    return {
      currentPeriod: {
        start: subscription.currentPeriodStart,
        end: subscription.currentPeriodEnd
      },
      nextBillingDate: subscription.billing.nextBillingDate,
      amount: subscription.billing.amount,
      currency: subscription.billing.currency,
      status: subscription.status
    };
  }

  // Calculate prorated refund
  calculateProratedRefund(subscriptionId: string): number {
    const subscription = this.subscriptions.get(subscriptionId);
    if (!subscription) {
      throw new Error(`Subscription ${subscriptionId} not found`);
    }

    const now = new Date();
    const periodStart = subscription.currentPeriodStart;
    const periodEnd = subscription.currentPeriodEnd;
    const totalPeriod = periodEnd.getTime() - periodStart.getTime();
    const remainingPeriod = periodEnd.getTime() - now.getTime();
    
    if (remainingPeriod <= 0) {
      return 0;
    }

    const proratedAmount = (remainingPeriod / totalPeriod) * subscription.billing.amount;
    return Math.round(proratedAmount * 100) / 100; // Round to 2 decimal places
  }
}

// Export singleton instance
export const subscriptionManager = new SubscriptionManager();
