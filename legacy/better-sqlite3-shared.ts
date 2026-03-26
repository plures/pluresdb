import { QueryResult } from "./types/node-types";

/**
 * Return `true` if `value` is a plain object (i.e. created by `{}` or
 * `Object.create(null)`), and `false` for arrays, class instances, `null`,
 * and primitives.
 *
 * Used throughout the better-sqlite3 compatibility layer to distinguish
 * named-parameter objects from positional-parameter arrays.
 *
 * @param value - The value to test.
 */
export function isPlainObject(
  value: unknown,
): value is Record<string, unknown> {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

/**
 * Normalise the `args` array passed to a better-sqlite3 statement runner into
 * the canonical form expected by PluresDB.
 *
 * - An empty array → returned as-is.
 * - A single array argument → unwrapped (the inner array becomes the params).
 * - A single plain-object argument → wrapped in an array (named params).
 * - Any other single value → wrapped in an array.
 * - Multiple arguments → returned unchanged.
 *
 * @param args - Raw arguments from the statement `.run()` / `.get()` call.
 * @returns Normalised parameter list.
 */
export function normalizeParameterInput(args: unknown[]): unknown[] {
  if (args.length === 0) {
    return [];
  }
  if (args.length === 1) {
    const first = args[0];
    if (Array.isArray(first)) {
      return first;
    }
    if (isPlainObject(first)) {
      return [first];
    }
    return [first];
  }
  return args;
}

function expandDotNotation(
  row: Record<string, unknown>,
): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(row)) {
    const parts = key.split(".");
    let cursor: Record<string, unknown> = result;
    for (let index = 0; index < parts.length; index++) {
      const part = parts[index];
      if (index === parts.length - 1) {
        cursor[part] = value;
      } else {
        const next = cursor[part];
        if (!isPlainObject(next)) {
          cursor[part] = {};
        }
        cursor = cursor[part] as Record<string, unknown>;
      }
    }
  }
  return result;
}

/**
 * Transform a raw database row into the shape requested by the caller.
 *
 * Applies the three display modes supported by the better-sqlite3 API:
 *
 * - **`raw`** – Return the row unchanged.
 * - **`pluck`** – Return only the first column's value as a scalar.
 * - **`expand`** – Expand dot-notation column names into nested objects
 *   (e.g. `"address.city"` → `{ address: { city: … } }`).
 *
 * If none of the modes are active the row is returned as a plain object with
 * column names as keys (arrays are zipped with `columns`; objects are shallow
 * cloned).
 *
 * @param row     - Raw row value from the underlying query engine.
 * @param columns - Ordered list of column names (used when `row` is an array).
 * @param mode    - Display mode flags.
 * @returns Transformed row value.
 */
export function shapeRow(
  row: unknown,
  columns: string[] | undefined,
  mode: { raw: boolean; pluck: boolean; expand: boolean },
): unknown {
  if (mode.raw) {
    return row;
  }

  let normalized: unknown;

  if (Array.isArray(row)) {
    if (columns && columns.length > 0) {
      const mapped: Record<string, unknown> = {};
      columns.forEach((column, index) => {
        mapped[column] = row[index];
      });
      normalized = mapped;
    } else {
      normalized = [...row];
    }
  } else if (isPlainObject(row)) {
    normalized = { ...(row as Record<string, unknown>) };
  } else {
    normalized = row;
  }

  if (mode.pluck) {
    if (Array.isArray(row)) {
      return row[0];
    }
    if (isPlainObject(row)) {
      const keys = Object.keys(row as Record<string, unknown>);
      return keys.length > 0
        ? (row as Record<string, unknown>)[keys[0]]
        : undefined;
    }
    if (columns && columns.length > 0 && isPlainObject(normalized)) {
      return (normalized as Record<string, unknown>)[columns[0]];
    }
    return normalized;
  }

  if (mode.expand && isPlainObject(normalized)) {
    return expandDotNotation(normalized as Record<string, unknown>);
  }

  return normalized;
}

/**
 * Coerce an unknown value returned by the PluresDB query engine into a
 * strongly-typed {@link QueryResult}.
 *
 * Handles three shapes:
 * - An object with a `rows` property → mapped field-by-field.
 * - An array → treated as a bare row list with no column metadata.
 * - `null` / `undefined` → returns an empty result.
 * - Any other value → wrapped in a single-element row list.
 *
 * @param raw - Raw value from the query engine.
 * @returns Normalised {@link QueryResult}.
 */
export function normalizeQueryResult(raw: unknown): QueryResult {
  if (
    raw && typeof raw === "object" && "rows" in (raw as Record<string, unknown>)
  ) {
    const result = raw as Partial<QueryResult> & Record<string, unknown>;
    const columnsValue = Array.isArray(result.columns) ? result.columns : [];
    const rowsValue = Array.isArray(result.rows) ? result.rows : [];
    const changesValue = typeof result.changes === "number"
      ? result.changes
      : 0;
    const lastInsertRowIdValue = typeof result.lastInsertRowId === "number"
      ? result.lastInsertRowId
      : typeof (result as Record<string, unknown>).lastInsertRowid === "number"
      ? Number((result as Record<string, unknown>).lastInsertRowid)
      : 0;

    return {
      rows: rowsValue,
      columns: columnsValue,
      changes: changesValue,
      lastInsertRowId: lastInsertRowIdValue,
    };
  }

  if (Array.isArray(raw)) {
    return {
      rows: raw,
      columns: [],
      changes: 0,
      lastInsertRowId: 0,
    };
  }

  if (raw === undefined || raw === null) {
    return { rows: [], columns: [], changes: 0, lastInsertRowId: 0 };
  }

  return {
    rows: [raw],
    columns: [],
    changes: 0,
    lastInsertRowId: 0,
  };
}

/**
 * Split a SQL string containing multiple statements separated by semicolons
 * into individual statement strings.
 *
 * Semicolons that appear inside single-quoted or double-quoted string literals
 * are treated as part of the literal and do not act as statement separators.
 * The returned strings are trimmed; empty strings are omitted.
 *
 * @param sql - One or more SQL statements joined by `;`.
 * @returns Array of individual SQL statement strings.
 */
export function splitSqlStatements(sql: string): string[] {
  return sql
    .split(/;\s*(?=(?:[^"']|"[^"]*"|'[^']*')*$)/)
    .map((statement) => statement.trim())
    .filter((statement) => statement.length > 0);
}

/**
 * Sanitize a string so it is safe to use as a filesystem directory name.
 *
 * Replaces whitespace and the path-separator characters `\`, `/`, and `:` with
 * underscores, then collapses consecutive underscores into a single one.
 *
 * @param name - Raw name to sanitize.
 * @returns Safe directory-name string.
 */
export function sanitizeDataDirName(name: string): string {
  return name.replace(/[\s\\/:]+/g, "_").replace(/_+/g, "_");
}
