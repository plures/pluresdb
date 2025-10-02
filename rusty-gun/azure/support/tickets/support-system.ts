// Rusty Gun Support Ticket System
// Handles customer support tickets, SLA monitoring, and knowledge base

export interface SupportTicket {
  id: string;
  customerId: string;
  subscriptionId?: string;
  title: string;
  description: string;
  priority: 'low' | 'medium' | 'high' | 'critical';
  status: 'open' | 'in_progress' | 'waiting_customer' | 'resolved' | 'closed';
  category: 'technical' | 'billing' | 'feature_request' | 'bug_report' | 'general';
  assignedTo?: string;
  createdAt: Date;
  updatedAt: Date;
  resolvedAt?: Date;
  closedAt?: Date;
  slaDeadline: Date;
  tags: string[];
  attachments: string[];
  messages: SupportMessage[];
  customer: {
    name: string;
    email: string;
    company?: string;
    plan?: string;
  };
}

export interface SupportMessage {
  id: string;
  ticketId: string;
  authorId: string;
  authorName: string;
  authorType: 'customer' | 'agent' | 'system';
  content: string;
  createdAt: Date;
  isInternal: boolean;
  attachments: string[];
}

export interface SLAConfig {
  priority: SupportTicket['priority'];
  responseTime: number; // in hours
  resolutionTime: number; // in hours
  escalationTime: number; // in hours
}

export interface SupportMetrics {
  totalTickets: number;
  openTickets: number;
  resolvedTickets: number;
  averageResponseTime: number; // in hours
  averageResolutionTime: number; // in hours
  customerSatisfactionScore: number; // 1-5
  slaCompliance: number; // percentage
}

export class SupportSystem {
  private tickets: Map<string, SupportTicket> = new Map();
  private messages: Map<string, SupportMessage> = new Map();
  private slaConfig: SLAConfig[] = [];
  private agents: Map<string, { id: string; name: string; email: string; skills: string[] }> = new Map();

  constructor() {
    this.initializeSLAConfig();
    this.initializeAgents();
  }

  private initializeSLAConfig(): void {
    this.slaConfig = [
      {
        priority: 'critical',
        responseTime: 1, // 1 hour
        resolutionTime: 4, // 4 hours
        escalationTime: 2 // 2 hours
      },
      {
        priority: 'high',
        responseTime: 4, // 4 hours
        resolutionTime: 24, // 24 hours
        escalationTime: 8 // 8 hours
      },
      {
        priority: 'medium',
        responseTime: 24, // 24 hours
        resolutionTime: 72, // 72 hours
        escalationTime: 48 // 48 hours
      },
      {
        priority: 'low',
        responseTime: 72, // 72 hours
        resolutionTime: 168, // 1 week
        escalationTime: 120 // 5 days
      }
    ];
  }

  private initializeAgents(): void {
    // Initialize with sample agents
    const agents = [
      {
        id: 'agent_1',
        name: 'Sarah Johnson',
        email: 'sarah.johnson@rusty-gun.com',
        skills: ['technical', 'billing', 'general']
      },
      {
        id: 'agent_2',
        name: 'Mike Chen',
        email: 'mike.chen@rusty-gun.com',
        skills: ['technical', 'bug_report', 'feature_request']
      },
      {
        id: 'agent_3',
        name: 'Emily Davis',
        email: 'emily.davis@rusty-gun.com',
        skills: ['billing', 'general', 'technical']
      }
    ];

    agents.forEach(agent => {
      this.agents.set(agent.id, agent);
    });
  }

  // Create a new support ticket
  async createTicket(
    customerId: string,
    title: string,
    description: string,
    priority: SupportTicket['priority'],
    category: SupportTicket['category'],
    customer: SupportTicket['customer'],
    subscriptionId?: string
  ): Promise<SupportTicket> {
    const ticketId = `ticket_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const now = new Date();
    
    // Calculate SLA deadline
    const slaConfig = this.slaConfig.find(sla => sla.priority === priority);
    const slaDeadline = new Date(now.getTime() + (slaConfig?.responseTime || 24) * 60 * 60 * 1000);

    const ticket: SupportTicket = {
      id: ticketId,
      customerId,
      subscriptionId,
      title,
      description,
      priority,
      status: 'open',
      category,
      createdAt: now,
      updatedAt: now,
      slaDeadline,
      tags: [],
      attachments: [],
      messages: [],
      customer
    };

    this.tickets.set(ticketId, ticket);

    // Auto-assign ticket based on priority and category
    await this.autoAssignTicket(ticketId);

    return ticket;
  }

  // Auto-assign ticket to appropriate agent
  private async autoAssignTicket(ticketId: string): Promise<void> {
    const ticket = this.tickets.get(ticketId);
    if (!ticket) return;

    // Find best agent based on skills and current workload
    const availableAgents = Array.from(this.agents.values())
      .filter(agent => agent.skills.includes(ticket.category))
      .map(agent => ({
        ...agent,
        workload: this.getAgentWorkload(agent.id)
      }))
      .sort((a, b) => a.workload - b.workload);

    if (availableAgents.length > 0) {
      ticket.assignedTo = availableAgents[0].id;
      ticket.status = 'in_progress';
      ticket.updatedAt = new Date();
    }
  }

  // Get agent's current workload
  private getAgentWorkload(agentId: string): number {
    return Array.from(this.tickets.values())
      .filter(ticket => ticket.assignedTo === agentId && ticket.status !== 'closed')
      .length;
  }

  // Get ticket by ID
  getTicket(ticketId: string): SupportTicket | undefined {
    return this.tickets.get(ticketId);
  }

  // Get tickets for a customer
  getCustomerTickets(customerId: string): SupportTicket[] {
    return Array.from(this.tickets.values())
      .filter(ticket => ticket.customerId === customerId)
      .sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
  }

  // Get tickets assigned to an agent
  getAgentTickets(agentId: string): SupportTicket[] {
    return Array.from(this.tickets.values())
      .filter(ticket => ticket.assignedTo === agentId)
      .sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
  }

  // Update ticket status
  async updateTicketStatus(
    ticketId: string,
    status: SupportTicket['status'],
    agentId?: string
  ): Promise<void> {
    const ticket = this.tickets.get(ticketId);
    if (!ticket) {
      throw new Error(`Ticket ${ticketId} not found`);
    }

    ticket.status = status;
    ticket.updatedAt = new Date();

    if (status === 'resolved') {
      ticket.resolvedAt = new Date();
    } else if (status === 'closed') {
      ticket.closedAt = new Date();
    }

    if (agentId) {
      ticket.assignedTo = agentId;
    }

    this.tickets.set(ticketId, ticket);
  }

  // Add message to ticket
  async addMessage(
    ticketId: string,
    authorId: string,
    authorName: string,
    authorType: SupportMessage['authorType'],
    content: string,
    isInternal: boolean = false,
    attachments: string[] = []
  ): Promise<SupportMessage> {
    const ticket = this.tickets.get(ticketId);
    if (!ticket) {
      throw new Error(`Ticket ${ticketId} not found`);
    }

    const messageId = `msg_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const message: SupportMessage = {
      id: messageId,
      ticketId,
      authorId,
      authorName,
      authorType,
      content,
      createdAt: new Date(),
      isInternal,
      attachments
    };

    this.messages.set(messageId, message);
    ticket.messages.push(message);
    ticket.updatedAt = new Date();

    // Update ticket status based on message type
    if (authorType === 'customer' && ticket.status === 'resolved') {
      ticket.status = 'open';
    } else if (authorType === 'agent' && ticket.status === 'open') {
      ticket.status = 'in_progress';
    }

    this.tickets.set(ticketId, ticket);
    return message;
  }

  // Get ticket messages
  getTicketMessages(ticketId: string): SupportMessage[] {
    return Array.from(this.messages.values())
      .filter(message => message.ticketId === ticketId)
      .sort((a, b) => a.createdAt.getTime() - b.createdAt.getTime());
  }

  // Check SLA compliance
  checkSLACompliance(): {
    compliant: number;
    nonCompliant: number;
    total: number;
    percentage: number;
  } {
    const now = new Date();
    const openTickets = Array.from(this.tickets.values())
      .filter(ticket => ticket.status !== 'closed' && ticket.status !== 'resolved');

    const compliant = openTickets.filter(ticket => ticket.slaDeadline > now).length;
    const nonCompliant = openTickets.filter(ticket => ticket.slaDeadline <= now).length;
    const total = openTickets.length;

    return {
      compliant,
      nonCompliant,
      total,
      percentage: total > 0 ? (compliant / total) * 100 : 100
    };
  }

  // Get support metrics
  getSupportMetrics(period: 'day' | 'week' | 'month' = 'month'): SupportMetrics {
    const now = new Date();
    const startDate = new Date(now);
    
    switch (period) {
      case 'day':
        startDate.setDate(startDate.getDate() - 1);
        break;
      case 'week':
        startDate.setDate(startDate.getDate() - 7);
        break;
      case 'month':
        startDate.setMonth(startDate.getMonth() - 1);
        break;
    }

    const ticketsInPeriod = Array.from(this.tickets.values())
      .filter(ticket => ticket.createdAt >= startDate);

    const totalTickets = ticketsInPeriod.length;
    const openTickets = ticketsInPeriod.filter(ticket => 
      ticket.status === 'open' || ticket.status === 'in_progress' || ticket.status === 'waiting_customer'
    ).length;
    const resolvedTickets = ticketsInPeriod.filter(ticket => ticket.status === 'resolved').length;

    // Calculate average response time
    const responseTimes = ticketsInPeriod
      .filter(ticket => ticket.messages.length > 1)
      .map(ticket => {
        const firstCustomerMessage = ticket.messages.find(msg => msg.authorType === 'customer');
        const firstAgentMessage = ticket.messages.find(msg => msg.authorType === 'agent');
        if (firstCustomerMessage && firstAgentMessage) {
          return firstAgentMessage.createdAt.getTime() - firstCustomerMessage.createdAt.getTime();
        }
        return 0;
      })
      .filter(time => time > 0);

    const averageResponseTime = responseTimes.length > 0 
      ? responseTimes.reduce((sum, time) => sum + time, 0) / responseTimes.length / (1000 * 60 * 60) // Convert to hours
      : 0;

    // Calculate average resolution time
    const resolutionTimes = ticketsInPeriod
      .filter(ticket => ticket.resolvedAt)
      .map(ticket => ticket.resolvedAt!.getTime() - ticket.createdAt.getTime())
      .filter(time => time > 0);

    const averageResolutionTime = resolutionTimes.length > 0
      ? resolutionTimes.reduce((sum, time) => sum + time, 0) / resolutionTimes.length / (1000 * 60 * 60) // Convert to hours
      : 0;

    // Mock customer satisfaction score (in real implementation, this would come from surveys)
    const customerSatisfactionScore = 4.2;

    const slaCompliance = this.checkSLACompliance().percentage;

    return {
      totalTickets,
      openTickets,
      resolvedTickets,
      averageResponseTime,
      averageResolutionTime,
      customerSatisfactionScore,
      slaCompliance
    };
  }

  // Get tickets by status
  getTicketsByStatus(status: SupportTicket['status']): SupportTicket[] {
    return Array.from(this.tickets.values())
      .filter(ticket => ticket.status === status)
      .sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
  }

  // Get tickets by priority
  getTicketsByPriority(priority: SupportTicket['priority']): SupportTicket[] {
    return Array.from(this.tickets.values())
      .filter(ticket => ticket.priority === priority)
      .sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
  }

  // Search tickets
  searchTickets(query: string): SupportTicket[] {
    const lowercaseQuery = query.toLowerCase();
    return Array.from(this.tickets.values())
      .filter(ticket => 
        ticket.title.toLowerCase().includes(lowercaseQuery) ||
        ticket.description.toLowerCase().includes(lowercaseQuery) ||
        ticket.customer.name.toLowerCase().includes(lowercaseQuery) ||
        ticket.customer.email.toLowerCase().includes(lowercaseQuery) ||
        ticket.tags.some(tag => tag.toLowerCase().includes(lowercaseQuery))
      )
      .sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime());
  }

  // Add tag to ticket
  addTag(ticketId: string, tag: string): void {
    const ticket = this.tickets.get(ticketId);
    if (!ticket) return;

    if (!ticket.tags.includes(tag)) {
      ticket.tags.push(tag);
      ticket.updatedAt = new Date();
      this.tickets.set(ticketId, ticket);
    }
  }

  // Remove tag from ticket
  removeTag(ticketId: string, tag: string): void {
    const ticket = this.tickets.get(ticketId);
    if (!ticket) return;

    ticket.tags = ticket.tags.filter(t => t !== tag);
    ticket.updatedAt = new Date();
    this.tickets.set(ticketId, ticket);
  }
}

// Export singleton instance
export const supportSystem = new SupportSystem();
