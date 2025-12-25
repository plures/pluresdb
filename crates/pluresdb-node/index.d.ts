export class PluresDatabase {
  constructor(actorId?: string);
  put(id: string, data: any): string;
  get(id: string): any | null;
  delete(id: string): void;
  list(): Array<{ id: string; data: any; timestamp: string }>;
  getActorId(): string;
}

export function init(): void;

