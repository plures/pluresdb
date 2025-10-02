// Rusty Gun Azure Cost Optimization System
// Monitors and optimizes Azure resource costs

export interface CostMetrics {
  resourceGroup: string;
  service: string;
  resourceName: string;
  cost: number;
  currency: string;
  period: 'daily' | 'weekly' | 'monthly';
  date: Date;
  tags: Record<string, string>;
}

export interface CostOptimization {
  id: string;
  type: 'rightsizing' | 'reserved' | 'spot' | 'scheduling' | 'cleanup';
  description: string;
  potentialSavings: number;
  currency: string;
  effort: 'low' | 'medium' | 'high';
  risk: 'low' | 'medium' | 'high';
  action: string;
  resources: string[];
}

export interface BudgetAlert {
  id: string;
  budgetName: string;
  threshold: number;
  currentSpend: number;
  currency: string;
  period: 'daily' | 'weekly' | 'monthly';
  alertType: 'warning' | 'critical';
  message: string;
  date: Date;
}

export class CostOptimizer {
  private costMetrics: CostMetrics[] = [];
  private optimizations: CostOptimization[] = [];
  private budgetAlerts: BudgetAlert[] = [];

  constructor() {
    this.initializeOptimizations();
  }

  private initializeOptimizations(): void {
    // Common cost optimizations for Rusty Gun
    const optimizations: CostOptimization[] = [
      {
        id: 'rightsize-app-service',
        type: 'rightsizing',
        description: 'Right-size App Service Plan based on actual usage',
        potentialSavings: 50,
        currency: 'USD',
        effort: 'low',
        risk: 'low',
        action: 'Monitor CPU and memory usage, downgrade if consistently underutilized',
        resources: ['App Service Plan']
      },
      {
        id: 'reserved-postgresql',
        type: 'reserved',
        description: 'Use Reserved Instances for PostgreSQL database',
        potentialSavings: 200,
        currency: 'USD',
        effort: 'medium',
        risk: 'low',
        action: 'Purchase 1-year or 3-year reserved instances for production databases',
        resources: ['PostgreSQL Database']
      },
      {
        id: 'schedule-dev-environments',
        type: 'scheduling',
        description: 'Schedule development environments to run only during business hours',
        potentialSavings: 300,
        currency: 'USD',
        effort: 'medium',
        risk: 'low',
        action: 'Use Azure Automation to start/stop dev environments outside business hours',
        resources: ['Development App Services', 'Development Databases']
      },
      {
        id: 'cleanup-unused-resources',
        type: 'cleanup',
        description: 'Remove unused or orphaned Azure resources',
        potentialSavings: 100,
        currency: 'USD',
        effort: 'low',
        risk: 'low',
        action: 'Regular cleanup of unused storage accounts, network interfaces, and other resources',
        resources: ['Storage Accounts', 'Network Interfaces', 'Public IPs']
      },
      {
        id: 'optimize-storage-tier',
        type: 'rightsizing',
        description: 'Optimize storage tier based on access patterns',
        potentialSavings: 150,
        currency: 'USD',
        effort: 'medium',
        risk: 'medium',
        action: 'Move infrequently accessed data to cooler storage tiers',
        resources: ['Storage Accounts']
      }
    ];

    optimizations.forEach(opt => {
      this.optimizations.push(opt);
    });
  }

  // Add cost metrics
  addCostMetrics(metrics: CostMetrics): void {
    this.costMetrics.push(metrics);
  }

  // Get cost metrics for a period
  getCostMetrics(
    resourceGroup?: string,
    period?: 'daily' | 'weekly' | 'monthly',
    startDate?: Date,
    endDate?: Date
  ): CostMetrics[] {
    let filtered = this.costMetrics;

    if (resourceGroup) {
      filtered = filtered.filter(m => m.resourceGroup === resourceGroup);
    }

    if (period) {
      filtered = filtered.filter(m => m.period === period);
    }

    if (startDate) {
      filtered = filtered.filter(m => m.date >= startDate);
    }

    if (endDate) {
      filtered = filtered.filter(m => m.date <= endDate);
    }

    return filtered;
  }

  // Calculate total cost for a period
  calculateTotalCost(
    resourceGroup?: string,
    period?: 'daily' | 'weekly' | 'monthly',
    startDate?: Date,
    endDate?: Date
  ): { total: number; currency: string; breakdown: Record<string, number> } {
    const metrics = this.getCostMetrics(resourceGroup, period, startDate, endDate);
    
    const total = metrics.reduce((sum, metric) => sum + metric.cost, 0);
    const currency = metrics[0]?.currency || 'USD';
    
    const breakdown: Record<string, number> = {};
    metrics.forEach(metric => {
      const key = `${metric.service}-${metric.resourceName}`;
      breakdown[key] = (breakdown[key] || 0) + metric.cost;
    });

    return { total, currency, breakdown };
  }

  // Get cost trends
  getCostTrends(
    resourceGroup?: string,
    period: 'daily' | 'weekly' | 'monthly' = 'daily',
    days: number = 30
  ): { date: string; cost: number; currency: string }[] {
    const endDate = new Date();
    const startDate = new Date(endDate);
    startDate.setDate(startDate.getDate() - days);

    const metrics = this.getCostMetrics(resourceGroup, period, startDate, endDate);
    
    // Group by date
    const grouped = new Map<string, number>();
    metrics.forEach(metric => {
      const dateKey = metric.date.toISOString().split('T')[0];
      grouped.set(dateKey, (grouped.get(dateKey) || 0) + metric.cost);
    });

    // Convert to array and sort by date
    return Array.from(grouped.entries())
      .map(([date, cost]) => ({
        date,
        cost,
        currency: metrics[0]?.currency || 'USD'
      }))
      .sort((a, b) => a.date.localeCompare(b.date));
  }

  // Get available optimizations
  getOptimizations(): CostOptimization[] {
    return this.optimizations;
  }

  // Get optimizations by type
  getOptimizationsByType(type: CostOptimization['type']): CostOptimization[] {
    return this.optimizations.filter(opt => opt.type === type);
  }

  // Get high-impact optimizations
  getHighImpactOptimizations(): CostOptimization[] {
    return this.optimizations
      .filter(opt => opt.potentialSavings > 100)
      .sort((a, b) => b.potentialSavings - a.potentialSavings);
  }

  // Get low-effort optimizations
  getLowEffortOptimizations(): CostOptimization[] {
    return this.optimizations
      .filter(opt => opt.effort === 'low')
      .sort((a, b) => b.potentialSavings - a.potentialSavings);
  }

  // Create budget alert
  createBudgetAlert(
    budgetName: string,
    threshold: number,
    currentSpend: number,
    currency: string = 'USD',
    period: 'daily' | 'weekly' | 'monthly' = 'monthly'
  ): BudgetAlert {
    const alert: BudgetAlert = {
      id: `alert_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`,
      budgetName,
      threshold,
      currentSpend,
      currency,
      period,
      alertType: currentSpend >= threshold * 0.9 ? 'critical' : 'warning',
      message: `Budget ${budgetName} is at ${((currentSpend / threshold) * 100).toFixed(1)}% of ${period} limit`,
      date: new Date()
    };

    this.budgetAlerts.push(alert);
    return alert;
  }

  // Get budget alerts
  getBudgetAlerts(alertType?: 'warning' | 'critical'): BudgetAlert[] {
    let filtered = this.budgetAlerts;
    
    if (alertType) {
      filtered = filtered.filter(alert => alert.alertType === alertType);
    }

    return filtered.sort((a, b) => b.date.getTime() - a.date.getTime());
  }

  // Get cost by service
  getCostByService(
    resourceGroup?: string,
    period?: 'daily' | 'weekly' | 'monthly',
    startDate?: Date,
    endDate?: Date
  ): Record<string, { cost: number; currency: string; count: number }> {
    const metrics = this.getCostMetrics(resourceGroup, period, startDate, endDate);
    const serviceCosts: Record<string, { cost: number; currency: string; count: number }> = {};

    metrics.forEach(metric => {
      if (!serviceCosts[metric.service]) {
        serviceCosts[metric.service] = {
          cost: 0,
          currency: metric.currency,
          count: 0
        };
      }
      serviceCosts[metric.service].cost += metric.cost;
      serviceCosts[metric.service].count += 1;
    });

    return serviceCosts;
  }

  // Get cost recommendations
  getCostRecommendations(): {
    immediate: CostOptimization[];
    shortTerm: CostOptimization[];
    longTerm: CostOptimization[];
  } {
    const immediate = this.optimizations
      .filter(opt => opt.effort === 'low' && opt.risk === 'low')
      .sort((a, b) => b.potentialSavings - a.potentialSavings)
      .slice(0, 3);

    const shortTerm = this.optimizations
      .filter(opt => opt.effort === 'medium' && opt.risk === 'low')
      .sort((a, b) => b.potentialSavings - a.potentialSavings)
      .slice(0, 3);

    const longTerm = this.optimizations
      .filter(opt => opt.effort === 'high' || opt.risk === 'high')
      .sort((a, b) => b.potentialSavings - a.potentialSavings)
      .slice(0, 3);

    return { immediate, shortTerm, longTerm };
  }

  // Calculate potential savings
  calculatePotentialSavings(): {
    total: number;
    currency: string;
    byType: Record<string, number>;
  } {
    const total = this.optimizations.reduce((sum, opt) => sum + opt.potentialSavings, 0);
    const currency = 'USD';
    
    const byType: Record<string, number> = {};
    this.optimizations.forEach(opt => {
      byType[opt.type] = (byType[opt.type] || 0) + opt.potentialSavings;
    });

    return { total, currency, byType };
  }
}

// Export singleton instance
export const costOptimizer = new CostOptimizer();
