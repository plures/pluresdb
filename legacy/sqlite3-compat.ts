// SQLite3 Compatible API for PluresDB
// Provides drop-in replacement for sqlite3 package

import { Database as PluresDBDatabase } from "./sqlite-compat.ts";

/**
 * Drop-in replacement for the `sqlite3` npm package's `Database` class.
 *
 * Extends {@link PluresDBDatabase} with the callback-style constructor and the
 * sqlite3-specific no-op methods (`serialize`, `parallelize`, `configure`,
 * `interrupt`, `loadExtension`) so that code written against the `sqlite3`
 * package can switch to PluresDB without API changes.
 *
 * @example
 * ```typescript
 * import { Database } from "pluresdb/sqlite3-compat";
 *
 * const db = new Database("./data.db", (err) => {
 *   if (err) console.error("Failed to open:", err);
 * });
 * db.run("INSERT INTO users VALUES (?, ?)", [1, "Alice"], (err) => { ... });
 * ```
 */
export class Database extends PluresDBDatabase {
  /**
   * Open a database at `filename`.
   *
   * If `callback` is provided it is invoked asynchronously once the database
   * is ready (or with an `Error` if opening fails), matching the sqlite3
   * package's callback-style constructor.
   *
   * @param filename - Path to the database file.
   * @param callback - Optional callback invoked when the database is ready.
   */
  constructor(filename: string, callback?: (err: Error | null) => void) {
    super({ filename });

    if (callback) {
      this.open()
        .then(() => callback(null))
        .catch((err) => callback(err));
    }
  }

  // SQLite3 specific methods

  /**
   * No-op — present for sqlite3 API compatibility.
   *
   * In the real sqlite3 package this forces sequential execution of subsequent
   * statements; PluresDB executes all statements sequentially by default.
   */
  serialize(callback?: (err: Error | null, sql: string) => void): void {
    if (callback) {
      callback(null, "-- Serialization not supported in PluresDB");
    }
  }

  /**
   * No-op — present for sqlite3 API compatibility.
   *
   * In the real sqlite3 package this allows parallel execution of subsequent
   * statements; PluresDB does not expose a parallel execution mode.
   */
  parallelize(callback?: (err: Error | null) => void): void {
    if (callback) {
      callback(null);
    }
  }

  /**
   * No-op — present for sqlite3 API compatibility.
   *
   * In the real sqlite3 package this sets low-level driver options.  PluresDB
   * has no equivalent configuration surface.
   */
  configure(_option: string, _value: any): void {
    // No-op for compatibility
  }

  /**
   * No-op — present for sqlite3 API compatibility.
   *
   * In the real sqlite3 package this cancels a pending long-running statement.
   * PluresDB does not expose statement cancellation.
   */
  interrupt(): void {
    // No-op for compatibility
  }

  /**
   * Not supported — always invokes `callback` with an error.
   *
   * Native extension loading is a sqlite3 feature that has no equivalent in
   * PluresDB.  Present to prevent crashes in code that conditionally loads
   * extensions.
   *
   * @param _path    - Path to the native extension (ignored).
   * @param callback - Invoked with an `Error` indicating lack of support.
   */
  loadExtension(_path: string, callback?: (err: Error | null) => void): void {
    if (callback) {
      callback(new Error("Extensions not supported in PluresDB"), null);
    }
  }
}

// sqlite3 open-mode flag constants (values mirror the real sqlite3 package)

/** Open the database in read-only mode. */
export const OPEN_READONLY = 1;
/** Open the database in read-write mode (fails if the file does not exist). */
export const OPEN_READWRITE = 2;
/** Create the database file if it does not exist. */
export const OPEN_CREATE = 4;
/** Enforce serialised threading mode. */
export const OPEN_FULLMUTEX = 0x00010000;
/** Enable shared cache mode. */
export const OPEN_SHAREDCACHE = 0x00020000;
/** Disable shared cache mode (private cache). */
export const OPEN_PRIVATECACHE = 0x00040000;
/** Allow URI filenames. */
export const OPEN_URI = 0x00000040;

export default Database;
