// SQLite Compatible API for PluresDB
// Provides drop-in replacement for sqlite3 and sqlite packages

import { dirname, resolve } from "node:path";

import { PluresNode } from "./node-wrapper.ts";

/**
 * Configuration for the `open()` factory function (mirrors the `sqlite` package API).
 */
export interface SQLiteConfig {
  /** Path to the SQLite database file. */
  filename: string;
  /** SQLite driver (accepted for API compatibility but ignored by PluresDB). */
  driver: any; // SQLite driver (ignored, but kept for compatibility)
  /** Open mode flags (accepted for API compatibility but not enforced). */
  mode?: number;
  /** When `true`, log SQL statements and row counts to `console.log`. */
  verbose?: boolean;
}

/**
 * Options accepted by the {@link Database} constructor.
 */
export interface DatabaseOptions {
  /** Path to the SQLite database file. */
  filename: string;
  /** SQLite driver (accepted for API compatibility but ignored by PluresDB). */
  driver?: any;
  /** Open mode flags (accepted for API compatibility but not enforced). */
  mode?: number;
  /** When `true`, log SQL statements and row counts to `console.log`. */
  verbose?: boolean;
}

type RowRecord = Record<string, unknown>;

/**
 * SQLite-compatible async database interface backed by PluresDB.
 *
 * Provides a drop-in replacement for the `sqlite` and `sqlite3` npm packages,
 * allowing existing code to switch to PluresDB without API changes.
 *
 * Only a subset of SQL is supported; complex queries may require migration.
 *
 * @example
 * ```typescript
 * const db = new Database({ filename: "./data.db" });
 * await db.open();
 * await db.exec("CREATE TABLE users (id INTEGER, name TEXT)");
 * await db.run("INSERT INTO users (id, name) VALUES (?, ?)", [1, "Alice"]);
 * const user = await db.get("SELECT * FROM users WHERE id = ?", [1]);
 * await db.close();
 * ```
 */
export class Database {
  private plures: PluresNode;
  private filename: string;
  private isOpen: boolean = false;
  private verbose: boolean = false;

  /**
   * Create a new database wrapper.
   *
   * The database is not ready until {@link open} resolves.
   *
   * @param options - Database options including the file path.
   */
  constructor(options: DatabaseOptions) {
    this.filename = options.filename;
    this.verbose = options.verbose || false;

    // Initialize PluresDB with the SQLite file path
    const dataDir = resolve(dirname(options.filename), "pluresdb");
    this.plures = new PluresNode({
      config: {
        dataDir,
        port: 34567,
        host: "localhost",
      },
    });
  }

  /**
   * Open the database connection.
   *
   * Starts the underlying PluresDB server.  Must be awaited before calling any
   * other method.
   *
   * @returns `this` for optional chaining.
   * @throws {Error} If the database fails to open.
   */
  async open(): Promise<Database> {
    if (this.isOpen) {
      return this;
    }

    try {
      await this.plures.start();
      this.isOpen = true;

      if (this.verbose) {
        console.log(`Database opened: ${this.filename}`);
      }

      return this;
    } catch (error) {
      throw new Error(`Failed to open database: ${formatError(error)}`);
    }
  }

  /**
   * Close the database connection.
   *
   * Stops the underlying PluresDB server.  Subsequent method calls will fail
   * until the database is re-opened.
   *
   * @throws {Error} If the database fails to close cleanly.
   */
  async close(): Promise<void> {
    if (!this.isOpen) {
      return;
    }

    try {
      await this.plures.stop();
      this.isOpen = false;

      if (this.verbose) {
        console.log(`Database closed: ${this.filename}`);
      }
    } catch (error) {
      throw new Error(`Failed to close database: ${formatError(error)}`);
    }
  }

  // SQLite-compatible methods
  /**
   * Execute one or more semicolon-separated SQL statements.
   *
   * Typically used for DDL statements such as `CREATE TABLE` or `DROP TABLE`.
   *
   * @param sql - One or more SQL statements separated by semicolons.
   * @throws {Error} If the database is not open or a statement fails.
   */
  async exec(sql: string): Promise<void> {
    if (!this.isOpen) {
      throw new Error("Database is not open");
    }

    try {
      // Parse SQL and execute
      const statements = this.parseSQL(sql);

      for (const statement of statements) {
        await this.executeStatement(statement);
      }
    } catch (error) {
      throw new Error(`SQL execution failed: ${formatError(error)}`);
    }
  }

  /**
   * Execute a single DML statement (INSERT, UPDATE, DELETE) with parameters.
   *
   * @param sql    - SQL statement with optional `?` placeholders.
   * @param params - Values to bind to the placeholders.
   * @returns Object with `lastID` (always 0) and `changes` (rows affected).
   * @throws {Error} If the database is not open or the statement fails.
   */
  async run(
    sql: string,
    params: any[] = [],
  ): Promise<{ lastID: number; changes: number }> {
    if (!this.isOpen) {
      throw new Error("Database is not open");
    }

    try {
      const result = await this.executeStatement({ sql, params });
      return {
        lastID: result.lastID || 0,
        changes: result.changes || 0,
      };
    } catch (error) {
      throw new Error(`SQL run failed: ${formatError(error)}`);
    }
  }

  /**
   * Execute a SELECT statement and return the **first** matching row.
   *
   * @param sql    - SELECT statement with optional `?` placeholders.
   * @param params - Values to bind to the placeholders.
   * @returns The first row as a plain object, or `undefined` if none found.
   * @throws {Error} If the database is not open or the query fails.
   */
  async get(sql: string, params: any[] = []): Promise<any> {
    if (!this.isOpen) {
      throw new Error("Database is not open");
    }

    try {
      const results = await this.executeQuery(sql, params);
      return results.length > 0 ? results[0] : undefined;
    } catch (error) {
      throw new Error(`SQL get failed: ${formatError(error)}`);
    }
  }

  /**
   * Execute a SELECT statement and return **all** matching rows.
   *
   * @param sql    - SELECT statement with optional `?` placeholders.
   * @param params - Values to bind to the placeholders.
   * @returns Array of row objects (may be empty).
   * @throws {Error} If the database is not open or the query fails.
   */
  async all(sql: string, params: any[] = []): Promise<any[]> {
    if (!this.isOpen) {
      throw new Error("Database is not open");
    }

    try {
      return await this.executeQuery(sql, params);
    } catch (error) {
      throw new Error(`SQL all failed: ${formatError(error)}`);
    }
  }

  /**
   * Execute a SELECT statement and invoke `callback` once per row.
   *
   * @param sql      - SELECT statement with optional `?` placeholders.
   * @param params   - Values to bind to the placeholders.
   * @param callback - Function called with each row object.
   * @returns The total number of rows processed.
   * @throws {Error} If the database is not open or the query fails.
   */
  async each(
    sql: string,
    params: any[] = [],
    callback: (row: any) => void,
  ): Promise<number> {
    if (!this.isOpen) {
      throw new Error("Database is not open");
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
      throw new Error(`SQL each failed: ${formatError(error)}`);
    }
  }

  // Transaction support
  /**
   * Execute `fn` inside a database transaction.
   *
   * Sends `BEGIN TRANSACTION` before calling `fn` and `COMMIT` on success.
   * If `fn` throws, `ROLLBACK` is sent and the error is re-thrown.
   *
   * @param fn - Async function that performs database operations.
   * @returns The value returned by `fn`.
   * @throws {Error} If the database is not open, or if `fn` throws.
   */
  async transaction<T>(fn: (db: Database) => Promise<T>): Promise<T> {
    if (!this.isOpen) {
      throw new Error("Database is not open");
    }

    try {
      await this.exec("BEGIN TRANSACTION");
      const result = await fn(this);
      await this.exec("COMMIT");
      return result;
    } catch (error) {
      await this.exec("ROLLBACK");
      throw error;
    }
  }

  // Prepare statements (simplified implementation)
  /**
   * Create a prepared statement that can be executed multiple times.
   *
   * @param sql - SQL statement with optional `?` placeholders.
   * @returns A {@link PreparedStatement} bound to this database.
   */
  prepare(sql: string): PreparedStatement {
    return new PreparedStatement(this, sql);
  }

  // Private helper methods
  private parseSQL(sql: string): Array<{ sql: string; params?: any[] }> {
    // Simple SQL parser - split by semicolon and trim
    const statements = sql
      .split(";")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);

    return statements.map((statement) => ({ sql: statement }));
  }

  private async executeStatement(
    statement: { sql: string; params?: any[] },
  ): Promise<any> {
    const sql = statement.sql.toLowerCase().trim();

    if (sql.startsWith("create table")) {
      return await this.createTable(statement.sql);
    } else if (sql.startsWith("drop table")) {
      return await this.dropTable(statement.sql);
    } else if (sql.startsWith("insert")) {
      return await this.insert(statement.sql, statement.params || []);
    } else if (sql.startsWith("update")) {
      return await this.update(statement.sql, statement.params || []);
    } else if (sql.startsWith("delete")) {
      return await this.delete(statement.sql, statement.params || []);
    } else if (
      sql.startsWith("begin") || sql.startsWith("commit") ||
      sql.startsWith("rollback")
    ) {
      // Transaction commands - handled by transaction method
      return { changes: 0 };
    } else {
      throw new Error(`Unsupported SQL statement: ${statement.sql}`);
    }
  }

  private async executeQuery(sql: string, params: any[]): Promise<RowRecord[]> {
    const sqlLower = sql.toLowerCase().trim();

    if (sqlLower.startsWith("select")) {
      return await this.select(sql, params);
    } else {
      throw new Error(`Unsupported query: ${sql}`);
    }
  }

  private async createTable(sql: string): Promise<void> {
    // Extract table name and columns from CREATE TABLE statement
    const tableMatch = sql.match(
      /CREATE TABLE\s+(?:IF NOT EXISTS\s+)?(\w+)\s*\(([^)]+)\)/i,
    );
    if (!tableMatch) {
      throw new Error(`Invalid CREATE TABLE statement: ${sql}`);
    }

    const tableName = tableMatch[1];
    const columns = tableMatch[2].split(",").map((col) => col.trim());

    // Store table schema in PluresDB
    await this.plures.put(`schema:${tableName}`, {
      name: tableName,
      columns: columns,
      created_at: new Date().toISOString(),
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
    await this.plures.delete(`schema:${tableName}`);

    // Delete all rows for this table
    const rows: RowRecord[] = await this.plures.query(`table:${tableName}:*`);
    for (const row of rows) {
      await this.plures.delete(getRowId(row));
    }

    if (this.verbose) {
      console.log(`Table dropped: ${tableName}`);
    }
  }

  private async insert(
    sql: string,
    params: any[],
  ): Promise<{ lastID: number; changes: number }> {
    const insertMatch = sql.match(
      /INSERT\s+(?:INTO\s+)?(\w+)\s*\(([^)]+)\)\s*VALUES\s*\(([^)]+)\)/i,
    );
    if (!insertMatch) {
      throw new Error(`Invalid INSERT statement: ${sql}`);
    }

    const tableName = insertMatch[1];
    const columns = insertMatch[2].split(",").map((col) => col.trim());
    const values = insertMatch[3].split(",").map((val) => val.trim());

    // Replace ? placeholders with actual values
    const actualValues = values.map((val, index) => {
      if (val === "?") {
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
    const id = `${tableName}:${Date.now()}:${
      Math.random().toString(36).substr(2, 9)
    }`;
    row.id = id;
    row.created_at = new Date().toISOString();

    // Store in PluresDB
    await this.plures.put(`table:${tableName}:${id}`, row);

    if (this.verbose) {
      console.log(`Row inserted into ${tableName}:`, row);
    }

    return { lastID: 0, changes: 1 };
  }

  private async update(
    sql: string,
    params: any[],
  ): Promise<{ changes: number }> {
    const updateMatch = sql.match(
      /UPDATE\s+(\w+)\s+SET\s+([^WHERE]+)(?:\s+WHERE\s+(.+))?/i,
    );
    if (!updateMatch) {
      throw new Error(`Invalid UPDATE statement: ${sql}`);
    }

    const tableName = updateMatch[1];
    const setClause = updateMatch[2];
    const whereClause = updateMatch[3];

    // Parse SET clause
    const setPairs = setClause.split(",").map((pair) => pair.trim());
    const updates: any = {};

    setPairs.forEach((pair, index) => {
      const [column, value] = pair.split("=").map((s) => s.trim());
      if (value === "?") {
        updates[column] = params[index] || null;
      } else if (value.startsWith("'") && value.endsWith("'")) {
        updates[column] = value.slice(1, -1);
      } else {
        updates[column] = value;
      }
    });

    // Find rows to update
    const rows: RowRecord[] = await this.plures.query(`table:${tableName}:*`);
    let changes = 0;

    for (const row of rows) {
      if (whereClause) {
        // Simple WHERE clause evaluation (basic implementation)
        if (this.evaluateWhereClause(row, whereClause, params)) {
          const updatedRow = {
            ...row,
            ...updates,
            updated_at: new Date().toISOString(),
          };
          await this.plures.put(getRowId(row), updatedRow);
          changes++;
        }
      } else {
        // Update all rows
        const updatedRow = {
          ...row,
          ...updates,
          updated_at: new Date().toISOString(),
        };
        await this.plures.put(getRowId(row), updatedRow);
        changes++;
      }
    }

    if (this.verbose) {
      console.log(`Updated ${changes} rows in ${tableName}`);
    }

    return { changes };
  }

  private async delete(
    sql: string,
    params: any[],
  ): Promise<{ changes: number }> {
    const deleteMatch = sql.match(/DELETE\s+FROM\s+(\w+)(?:\s+WHERE\s+(.+))?/i);
    if (!deleteMatch) {
      throw new Error(`Invalid DELETE statement: ${sql}`);
    }

    const tableName = deleteMatch[1];
    const whereClause = deleteMatch[2];

    // Find rows to delete
    const rows: RowRecord[] = await this.plures.query(`table:${tableName}:*`);
    let changes = 0;

    for (const row of rows) {
      if (whereClause) {
        // Simple WHERE clause evaluation
        if (this.evaluateWhereClause(row, whereClause, params)) {
          await this.plures.delete(getRowId(row));
          changes++;
        }
      } else {
        // Delete all rows
        await this.plures.delete(getRowId(row));
        changes++;
      }
    }

    if (this.verbose) {
      console.log(`Deleted ${changes} rows from ${tableName}`);
    }

    return { changes };
  }

  private async select(sql: string, params: any[]): Promise<RowRecord[]> {
    const selectMatch = sql.match(
      /SELECT\s+(.+?)\s+FROM\s+(\w+)(?:\s+WHERE\s+(.+))?(?:\s+ORDER\s+BY\s+(.+))?(?:\s+LIMIT\s+(\d+))?/i,
    );
    if (!selectMatch) {
      throw new Error(`Invalid SELECT statement: ${sql}`);
    }

    const columns = selectMatch[1];
    const tableName = selectMatch[2];
    const whereClause = selectMatch[3];
    const orderBy = selectMatch[4];
    const limit = selectMatch[5] ? parseInt(selectMatch[5]) : undefined;

    // Get all rows for the table
    const rows: RowRecord[] = await this.plures.query(`table:${tableName}:*`);
    let results: RowRecord[] = rows;

    // Apply WHERE clause
    if (whereClause) {
      results = results.filter((row) =>
        this.evaluateWhereClause(row, whereClause, params)
      );
    }

    // Apply ORDER BY
    if (orderBy) {
      const [column, direction] = orderBy.split(/\s+/);
      const isDesc = direction?.toLowerCase() === "desc";
      results.sort((a, b) => compareValues(a[column], b[column], isDesc));
    }

    // Apply LIMIT
    if (limit) {
      results = results.slice(0, limit);
    }

    // Select specific columns
    if (columns !== "*") {
      const columnList = columns.split(",").map((col) => col.trim());
      results = results.map((row) => {
        const selectedRow: RowRecord = {};
        columnList.forEach((col) => {
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

  private evaluateWhereClause(
    row: RowRecord,
    whereClause: string,
    params: any[],
  ): boolean {
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

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return "Unknown error";
  }
}

function getRowId(row: RowRecord): string {
  const id = row["id"];
  if (typeof id === "string" && id.length > 0) {
    return id;
  }
  throw new Error("Row is missing a valid string id");
}

function compareValues(a: unknown, b: unknown, desc = false): number {
  const normalize = (value: unknown): string => {
    if (value === null || value === undefined) {
      return "";
    }
    if (typeof value === "string") {
      return value;
    }
    if (
      typeof value === "number" || typeof value === "boolean" ||
      typeof value === "bigint"
    ) {
      return value.toString();
    }
    try {
      return JSON.stringify(value);
    } catch {
      return String(value);
    }
  };

  const normalizedA = normalize(a);
  const normalizedB = normalize(b);
  const result = normalizedA.localeCompare(normalizedB, undefined, {
    numeric: true,
    sensitivity: "base",
  });

  return desc ? -result : result;
}

/**
 * A pre-compiled SQL statement that can be executed multiple times with
 * different parameter bindings.
 *
 * Obtain an instance via {@link Database.prepare}.
 */
export class PreparedStatement {
  private db: Database;
  private sql: string;

  /**
   * Create a prepared statement.
   *
   * @param db  - Parent database instance.
   * @param sql - SQL template string.
   */
  constructor(db: Database, sql: string) {
    this.db = db;
    this.sql = sql;
  }

  /**
   * Execute the statement as a DML command.
   *
   * @param params - Parameter bindings for `?` placeholders.
   * @returns Object with `lastID` and `changes`.
   */
  async run(params: any[] = []): Promise<{ lastID: number; changes: number }> {
    return await this.db.run(this.sql, params);
  }

  /**
   * Execute the statement as a SELECT query and return the first row.
   *
   * @param params - Parameter bindings for `?` placeholders.
   * @returns The first matching row, or `undefined` if none found.
   */
  async get(params: any[] = []): Promise<any> {
    return await this.db.get(this.sql, params);
  }

  /**
   * Execute the statement as a SELECT query and return all rows.
   *
   * @param params - Parameter bindings for `?` placeholders.
   * @returns Array of matching row objects.
   */
  async all(params: any[] = []): Promise<any[]> {
    return await this.db.all(this.sql, params);
  }

  /**
   * Execute the statement as a SELECT query and invoke `callback` per row.
   *
   * @param params   - Parameter bindings for `?` placeholders.
   * @param callback - Called once for each matching row.
   * @returns Total number of rows processed.
   */
  async each(
    params: any[] = [],
    callback: (row: any) => void,
  ): Promise<number> {
    return await this.db.each(this.sql, params, callback);
  }

  /** No-op. Included for API compatibility with the `sqlite3` package. */
  finalize(): void {
    // No-op for compatibility
  }
}

// SQLite3 driver compatibility
/**
 * Drop-in replacement for the `sqlite3` npm package `Database` class.
 *
 * Extends {@link Database} with a callback-based constructor to match the
 * `sqlite3` API and exposes the standard open-mode constants.
 *
 * @example
 * ```typescript
 * const db = new SQLite3Database("./data.db", (err) => {
 *   if (err) throw err;
 *   // database is now open
 * });
 * ```
 */
export class SQLite3Database extends Database {
  /**
   * @param filename - Path to the database file.
   * @param callback - Optional callback invoked when the database is opened.
   */
  constructor(filename: string, callback?: (err: Error | null) => void) {
    super({ filename });

    if (callback) {
      this.open()
        .then(() => callback(null))
        .catch((err) => callback(err));
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
/**
 * Open a database connection (mirrors the `sqlite` package `open()` API).
 *
 * @param options - Configuration including the `filename` to open.
 * @returns A fully opened {@link Database} instance.
 * @throws {Error} If the database fails to open.
 */
export async function open(options: SQLiteConfig): Promise<Database> {
  const db = new Database(options);
  return await db.open();
}

// Export for compatibility with sqlite package
export { Database as default };
