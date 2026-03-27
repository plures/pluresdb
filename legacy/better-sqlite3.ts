/**
 * `better-sqlite3`-compatible entry point for PluresDB.
 *
 * Re-exports {@link BetterSQLite3Database} as the default export so that code
 * written against the `better-sqlite3` npm package can drop in PluresDB with
 * minimal changes.
 *
 * @example
 * ```typescript
 * import Database from "pluresdb/better-sqlite3";
 *
 * const db = new Database(":memory:");
 * db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)");
 * db.prepare("INSERT INTO users VALUES (?, ?)").run(1, "Alice");
 * const row = db.prepare("SELECT name FROM users WHERE id = ?").get(1);
 * console.log(row.name); // "Alice"
 * ```
 */
import { BetterSQLite3Database, BetterSQLite3Statement } from "./node-index";

export default BetterSQLite3Database;
export { BetterSQLite3Database, BetterSQLite3Statement };
