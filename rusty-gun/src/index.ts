/**
 * Main entry point for Rusty Gun Node.js package
 * This provides a clean API for VSCode extensions and other Node.js applications
 */

export { RustyGunNode as default, RustyGunNode } from './node-wrapper';
export type { RustyGunConfig, RustyGunOptions } from './node-wrapper';

// Re-export types from the main types file
export * from './types/index';

// Convenience function to create a new RustyGun instance
export function createRustyGun(options?: import('./node-wrapper').RustyGunOptions) {
  return new (require('./node-wrapper').RustyGunNode)(options);
}

// SQLite-compatible API for easy migration
export class SQLiteCompatibleAPI {
  private rustyGun: import('./node-wrapper').RustyGunNode;

  constructor(options?: import('./node-wrapper').RustyGunOptions) {
    this.rustyGun = new (require('./node-wrapper').RustyGunNode)(options);
  }

  async start() {
    await this.rustyGun.start();
  }

  async stop() {
    await this.rustyGun.stop();
  }

  // SQLite-compatible methods
  async run(sql: string, params: any[] = []) {
    return this.rustyGun.query(sql, params);
  }

  async get(sql: string, params: any[] = []) {
    const result = await this.rustyGun.query(sql, params);
    return result.rows?.[0] || null;
  }

  async all(sql: string, params: any[] = []) {
    const result = await this.rustyGun.query(sql, params);
    return result.rows || [];
  }

  async exec(sql: string) {
    return this.rustyGun.query(sql);
  }

  // Additional Rusty Gun specific methods
  async put(key: string, value: any) {
    return this.rustyGun.put(key, value);
  }

  async getValue(key: string) {
    return this.rustyGun.get(key);
  }

  async delete(key: string) {
    return this.rustyGun.delete(key);
  }

  async vectorSearch(query: string, limit = 10) {
    return this.rustyGun.vectorSearch(query, limit);
  }

  async list(prefix?: string) {
    return this.rustyGun.list(prefix);
  }

  getApiUrl() {
    return this.rustyGun.getApiUrl();
  }

  getWebUrl() {
    return this.rustyGun.getWebUrl();
  }

  isRunning() {
    return this.rustyGun.isServerRunning();
  }
}

// Export the SQLite-compatible API as well
export { SQLiteCompatibleAPI };

