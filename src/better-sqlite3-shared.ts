import { QueryResult } from "./types/node-types";

export function isPlainObject(value: unknown): value is Record<string, unknown> {
  if (value === null || typeof value !== "object") {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}

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

function expandDotNotation(row: Record<string, unknown>): Record<string, unknown> {
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
      return keys.length > 0 ? (row as Record<string, unknown>)[keys[0]] : undefined;
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

export function normalizeQueryResult(raw: unknown): QueryResult {
  if (raw && typeof raw === "object" && "rows" in (raw as Record<string, unknown>)) {
    const result = raw as Partial<QueryResult> & Record<string, unknown>;
    const columnsValue = Array.isArray(result.columns) ? result.columns : [];
    const rowsValue = Array.isArray(result.rows) ? result.rows : [];
    const changesValue = typeof result.changes === "number" ? result.changes : 0;
    const lastInsertRowIdValue =
      typeof result.lastInsertRowId === "number"
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

export function splitSqlStatements(sql: string): string[] {
  return sql
    .split(/;\s*(?=(?:[^"']|"[^"]*"|'[^']*')*$)/)
    .map((statement) => statement.trim())
    .filter((statement) => statement.length > 0);
}

export function sanitizeDataDirName(name: string): string {
  return name.replace(/[\s\\/:]+/g, "_").replace(/_+/g, "_");
}
