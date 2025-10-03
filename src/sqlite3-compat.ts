// SQLite3 Compatible API for PluresDB
// Provides drop-in replacement for sqlite3 package

import { Database as PluresDBDatabase } from './sqlite-compat.ts';

export class Database extends PluresDBDatabase {
  constructor(filename: string, callback?: (err: Error | null) => void) {
    super({ filename });
    
    if (callback) {
      this.open().then(() => callback(null)).catch(err => callback(err));
    }
  }

  // SQLite3 specific methods
  serialize(callback?: (err: Error | null, sql: string) => void): void {
    if (callback) {
      callback(null, '-- Serialization not supported in PluresDB');
    }
  }

  parallelize(callback?: (err: Error | null) => void): void {
    if (callback) {
      callback(null);
    }
  }

  configure(option: string, value: any): void {
    // No-op for compatibility
  }

  interrupt(): void {
    // No-op for compatibility
  }

  loadExtension(path: string, callback?: (err: Error | null) => void): void {
    if (callback) {
      callback(new Error('Extensions not supported in PluresDB'), null);
    }
  }
}

// Export constants for compatibility
export const OPEN_READONLY = 1;
export const OPEN_READWRITE = 2;
export const OPEN_CREATE = 4;
export const OPEN_FULLMUTEX = 0x00010000;
export const OPEN_SHAREDCACHE = 0x00020000;
export const OPEN_PRIVATECACHE = 0x00040000;
export const OPEN_URI = 0x00000040;

// Export Database as default
export default Database;
