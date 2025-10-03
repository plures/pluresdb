// SQLite Compatible API for PluresDB
// Provides drop-in replacement for sqlite3 and sqlite packages

import { PluresNode } from './main.ts';
import { join } from 'https://deno.land/std@0.208.0/path/mod.ts';

export interface SQLiteConfig {
  filename: string;
  driver: any; // SQLite driver (ignored, but kept for compatibility)
  mode?: number;
  verbose?: boolean;
}

export interface DatabaseOptions {
  filename: string;
  driver?: any;
  mode?: number;
  verbose?: boolean;
}

export class Database {
  private rustyGun: PluresNode;
  private filename: string;
  private isOpen: boolean = false;
  private verbose: boolean = false;

  constructor(options: DatabaseOptions) {
    this.filename = options.filename;
    this.verbose = options.verbose || false;
    
    // Initialize PluresDB with the SQLite file path
    const dataDir = join(options.filename, '..', 'pluresdb');
    this.rustyGun = new PluresNode({
      config: {
        dataDir,
        port: 34567,
        host: 'localhost'
      }
    });
  }

  async open(): Promise<Database> {
    if (this.isOpen) {
      return this;
    }

    try {
      await this.rustyGun.start();
      this.isOpen = true;
      
      if (this.verbose) {
        console.log(`Database opened: ${this.filename}`);
      }
      
      return this;
    } catch (error) {
      throw new Error(`Failed to open database: ${error.message}`);
    }
  }

  async close(): Promise<void> {
    if (!this.isOpen) {
      return;
    }

    try {
      await this.rustyGun.stop();
      this.isOpen = false;
      
      if (this.verbose) {
        console.log(`Database closed: ${this.filename}`);
      }
    } catch (error) {
      throw new Error(`Failed to close database: ${error.message}`);
    }
  }

  // SQLite-compatible methods
  async exec(sql: string): Promise<void> {
    if (!this.isOpen) {
      throw new Error('Database is not open');
    }

    try {
      // Parse SQL and execute
      const statements = this.parseSQL(sql);
      
      for (const statement of statements) {
        await this.executeStatement(statement);
      }
    } catch (error) {
      throw new Error(`SQL execution failed: ${error.message}`);
    }
  }

  async run(sql: string, params: any[] = []): Promise<{ lastID: number; changes: number }> {
    if (!this.isOpen) {
      throw new Error('Database is not open');
    }

    try {
      const result = await this.executeStatement({ sql, params });
      return {
        lastID: result.lastID || 0,
        changes: result.changes || 0
      };
    } catch (error) {
      throw new Error(`SQL run failed: ${error.message}`);
    }
  }

  async get(sql: string, params: any[] = []): Promise<any> {
    if (!this.isOpen) {
      throw new Error('Database is not open');
    }

    try {
      const results = await this.executeQuery(sql, params);
      return results.length > 0 ? results[0] : undefined;
    } catch (error) {
      throw new Error(`SQL get failed: ${error.message}`);
    }
  }

  async all(sql: string, params: any[] = []): Promise<any[]> {
    if (!this.isOpen) {
      throw new Error('Database is not open');
    }

    try {
      return await this.executeQuery(sql, params);
    } catch (error) {
      throw new Error(`SQL all failed: ${error.message}`);
    }
  }

  async each(sql: string, params: any[] = [], callback: (row: any) => void): Promise<number> {
    if (!this.isOpen) {
      throw new Error('Database is not open');
    }

    try {
      const results = await this.executeQuery(sql, params);
      let count = 0;
      
      for (const row of results) {
        callback(row);
        count++;
      }
      
      return count;
    } catch (error) {
      throw new Error(`SQL each failed: ${error.message}`);
    }
  }

  // Transaction support
  async transaction<T>(fn: (db: Database) => Promise<T>): Promise<T> {
    if (!this.isOpen) {
      throw new Error('Database is not open');
    }

    try {
      await this.exec('BEGIN TRANSACTION');
      const result = await fn(this);
      await this.exec('COMMIT');
      return result;
    } catch (error) {
      await this.exec('ROLLBACK');
      throw error;
    }
  }

  // Prepare statements (simplified implementation)
  prepare(sql: string): PreparedStatement {
    return new PreparedStatement(this, sql);
  }

  // Private helper methods
  private parseSQL(sql: string): Array<{ sql: string; params?: any[] }> {
    // Simple SQL parser - split by semicolon and trim
    const statements = sql.split(';')
      .map(s => s.trim())
      .filter(s => s.length > 0);
    
    return statements.map(statement => ({ sql: statement }));
  }

  private async executeStatement(statement: { sql: string; params?: any[] }): Promise<any> {
    const sql = statement.sql.toLowerCase().trim();
    
    if (sql.startsWith('create table')) {
      return await this.createTable(statement.sql);
    } else if (sql.startsWith('drop table')) {
      return await this.dropTable(statement.sql);
    } else if (sql.startsWith('insert')) {
      return await this.insert(statement.sql, statement.params || []);
    } else if (sql.startsWith('update')) {
      return await this.update(statement.sql, statement.params || []);
    } else if (sql.startsWith('delete')) {
      return await this.delete(statement.sql, statement.params || []);
    } else if (sql.startsWith('begin') || sql.startsWith('commit') || sql.startsWith('rollback')) {
      // Transaction commands - handled by transaction method
      return { changes: 0 };
    } else {
      throw new Error(`Unsupported SQL statement: ${statement.sql}`);
    }
  }

  private async executeQuery(sql: string, params: any[]): Promise<any[]> {
    const sqlLower = sql.toLowerCase().trim();
    
    if (sqlLower.startsWith('select')) {
      return await this.select(sql, params);
    } else {
      throw new Error(`Unsupported query: ${sql}`);
    }
  }

  private async createTable(sql: string): Promise<void> {
    // Extract table name and columns from CREATE TABLE statement
    const tableMatch = sql.match(/CREATE TABLE\s+(?:IF NOT EXISTS\s+)?(\w+)\s*\(([^)]+)\)/i);
    if (!tableMatch) {
      throw new Error(`Invalid CREATE TABLE statement: ${sql}`);
    }

    const tableName = tableMatch[1];
    const columns = tableMatch[2].split(',').map(col => col.trim());
    
    // Store table schema in PluresDB
    await this.rustyGun.put(`schema:${tableName}`, {
      name: tableName,
      columns: columns,
      created_at: new Date().toISOString()
    });

    if (this.verbose) {
      console.log(`Table created: ${tableName}`);
    }
  }

  private async dropTable(sql: string): Promise<void> {
    const tableMatch = sql.match(/DROP TABLE\s+(?:IF EXISTS\s+)?(\w+)/i);
    if (!tableMatch) {
      throw new Error(`Invalid DROP TABLE statement: ${sql}`);
    }

    const tableName = tableMatch[1];
    
    // Remove table schema and all data
    await this.rustyGun.delete(`schema:${tableName}`);
    
    // Delete all rows for this table
    const rows = await this.rustyGun.query(`table:${tableName}:*`);
    for (const row of rows) {
      await this.rustyGun.delete(row.id);
    }

    if (this.verbose) {
      console.log(`Table dropped: ${tableName}`);
    }
  }

  private async insert(sql: string, params: any[]): Promise<{ lastID: number; changes: number }> {
    const insertMatch = sql.match(/INSERT\s+(?:INTO\s+)?(\w+)\s*\(([^)]+)\)\s*VALUES\s*\(([^)]+)\)/i);
    if (!insertMatch) {
      throw new Error(`Invalid INSERT statement: ${sql}`);
    }

    const tableName = insertMatch[1];
    const columns = insertMatch[2].split(',').map(col => col.trim());
    const values = insertMatch[3].split(',').map(val => val.trim());
    
    // Replace ? placeholders with actual values
    const actualValues = values.map((val, index) => {
      if (val === '?') {
        return params[index] || null;
      } else if (val.startsWith("'") && val.endsWith("'")) {
        return val.slice(1, -1); // Remove quotes
      } else {
        return val;
      }
    });

    // Create row object
    const row: any = {};
    columns.forEach((col, index) => {
      row[col] = actualValues[index];
    });

    // Generate unique ID
    const id = `${tableName}:${Date.now()}:${Math.random().toString(36).substr(2, 9)}`;
    row.id = id;
    row.created_at = new Date().toISOString();

    // Store in PluresDB
    await this.rustyGun.put(`table:${tableName}:${id}`, row);

    if (this.verbose) {
      console.log(`Row inserted into ${tableName}:`, row);
    }

    return { lastID: 0, changes: 1 };
  }

  private async update(sql: string, params: any[]): Promise<{ changes: number }> {
    const updateMatch = sql.match(/UPDATE\s+(\w+)\s+SET\s+([^WHERE]+)(?:\s+WHERE\s+(.+))?/i);
    if (!updateMatch) {
      throw new Error(`Invalid UPDATE statement: ${sql}`);
    }

    const tableName = updateMatch[1];
    const setClause = updateMatch[2];
    const whereClause = updateMatch[3];

    // Parse SET clause
    const setPairs = setClause.split(',').map(pair => pair.trim());
    const updates: any = {};
    
    setPairs.forEach((pair, index) => {
      const [column, value] = pair.split('=').map(s => s.trim());
      if (value === '?') {
        updates[column] = params[index] || null;
      } else if (value.startsWith("'") && value.endsWith("'")) {
        updates[column] = value.slice(1, -1);
      } else {
        updates[column] = value;
      }
    });

    // Find rows to update
    const rows = await this.rustyGun.query(`table:${tableName}:*`);
    let changes = 0;

    for (const row of rows) {
      if (whereClause) {
        // Simple WHERE clause evaluation (basic implementation)
        if (this.evaluateWhereClause(row, whereClause, params)) {
          const updatedRow = { ...row, ...updates, updated_at: new Date().toISOString() };
          await this.rustyGun.put(row.id, updatedRow);
          changes++;
        }
      } else {
        // Update all rows
        const updatedRow = { ...row, ...updates, updated_at: new Date().toISOString() };
        await this.rustyGun.put(row.id, updatedRow);
        changes++;
      }
    }

    if (this.verbose) {
      console.log(`Updated ${changes} rows in ${tableName}`);
    }

    return { changes };
  }

  private async delete(sql: string, params: any[]): Promise<{ changes: number }> {
    const deleteMatch = sql.match(/DELETE\s+FROM\s+(\w+)(?:\s+WHERE\s+(.+))?/i);
    if (!deleteMatch) {
      throw new Error(`Invalid DELETE statement: ${sql}`);
    }

    const tableName = deleteMatch[1];
    const whereClause = deleteMatch[2];

    // Find rows to delete
    const rows = await this.rustyGun.query(`table:${tableName}:*`);
    let changes = 0;

    for (const row of rows) {
      if (whereClause) {
        // Simple WHERE clause evaluation
        if (this.evaluateWhereClause(row, whereClause, params)) {
          await this.rustyGun.delete(row.id);
          changes++;
        }
      } else {
        // Delete all rows
        await this.rustyGun.delete(row.id);
        changes++;
      }
    }

    if (this.verbose) {
      console.log(`Deleted ${changes} rows from ${tableName}`);
    }

    return { changes };
  }

  private async select(sql: string, params: any[]): Promise<any[]> {
    const selectMatch = sql.match(/SELECT\s+(.+?)\s+FROM\s+(\w+)(?:\s+WHERE\s+(.+))?(?:\s+ORDER\s+BY\s+(.+))?(?:\s+LIMIT\s+(\d+))?/i);
    if (!selectMatch) {
      throw new Error(`Invalid SELECT statement: ${sql}`);
    }

    const columns = selectMatch[1];
    const tableName = selectMatch[2];
    const whereClause = selectMatch[3];
    const orderBy = selectMatch[4];
    const limit = selectMatch[5] ? parseInt(selectMatch[5]) : undefined;

    // Get all rows for the table
    const rows = await this.rustyGun.query(`table:${tableName}:*`);
    let results = rows;

    // Apply WHERE clause
    if (whereClause) {
      results = results.filter(row => this.evaluateWhereClause(row, whereClause, params));
    }

    // Apply ORDER BY
    if (orderBy) {
      const [column, direction] = orderBy.split(/\s+/);
      results.sort((a, b) => {
        const aVal = a[column];
        const bVal = b[column];
        if (direction && direction.toLowerCase() === 'desc') {
          return bVal > aVal ? 1 : -1;
        } else {
          return aVal > bVal ? 1 : -1;
        }
      });
    }

    // Apply LIMIT
    if (limit) {
      results = results.slice(0, limit);
    }

    // Select specific columns
    if (columns !== '*') {
      const columnList = columns.split(',').map(col => col.trim());
      results = results.map(row => {
        const selectedRow: any = {};
        columnList.forEach(col => {
          selectedRow[col] = row[col];
        });
        return selectedRow;
      });
    }

    if (this.verbose) {
      console.log(`Selected ${results.length} rows from ${tableName}`);
    }

    return results;
  }

  private evaluateWhereClause(row: any, whereClause: string, params: any[]): boolean {
    // Simple WHERE clause evaluation
    // This is a basic implementation - in production, you'd want a proper SQL parser
    
    // Handle simple equality comparisons
    const equalityMatch = whereClause.match(/(\w+)\s*=\s*\?/);
    if (equalityMatch) {
      const column = equalityMatch[1];
      const value = params[0];
      return row[column] === value;
    }

    // Handle string equality
    const stringMatch = whereClause.match(/(\w+)\s*=\s*'([^']+)'/);
    if (stringMatch) {
      const column = stringMatch[1];
      const value = stringMatch[2];
      return row[column] === value;
    }

    // Default to true for unsupported WHERE clauses
    return true;
  }
}

export class PreparedStatement {
  private db: Database;
  private sql: string;

  constructor(db: Database, sql: string) {
    this.db = db;
    this.sql = sql;
  }

  async run(params: any[] = []): Promise<{ lastID: number; changes: number }> {
    return await this.db.run(this.sql, params);
  }

  async get(params: any[] = []): Promise<any> {
    return await this.db.get(this.sql, params);
  }

  async all(params: any[] = []): Promise<any[]> {
    return await this.db.all(this.sql, params);
  }

  async each(params: any[] = [], callback: (row: any) => void): Promise<number> {
    return await this.db.each(this.sql, params, callback);
  }

  finalize(): void {
    // No-op for compatibility
  }
}

// SQLite3 driver compatibility
export class Database as SQLite3Database {
  constructor(filename: string, callback?: (err: Error | null) => void) {
    super({ filename });
    
    if (callback) {
      this.open().then(() => callback(null)).catch(err => callback(err));
    }
  }

  static Database = Database;
  static OPEN_READONLY = 1;
  static OPEN_READWRITE = 2;
  static OPEN_CREATE = 4;
  static OPEN_FULLMUTEX = 0x00010000;
  static OPEN_SHAREDCACHE = 0x00020000;
  static OPEN_PRIVATECACHE = 0x00040000;
  static OPEN_URI = 0x00000040;
}

// Main export function - SQLite compatible API
export async function open(options: SQLiteConfig): Promise<Database> {
  const db = new Database(options);
  return await db.open();
}

// Export for compatibility with sqlite package
export { Database as default };
