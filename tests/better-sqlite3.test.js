const test = require("node:test");
const assert = require("node:assert/strict");

const { BetterSQLite3Statement } = require("../dist/node-index.js");

function createStatementWithResult(result, sql = "SELECT 1") {
  const fakeDatabase = {
    executeStatement: async () => result,
  };
  // Cast to expected shape
  return new BetterSQLite3Statement(fakeDatabase, sql);
}

test("BetterSQLite3Statement all() maps rows to objects", async () => {
  const statement = createStatementWithResult({
    rows: [
      [1, "Alice"],
      [2, "Bob"],
    ],
    columns: ["id", "name"],
    changes: 0,
    lastInsertRowId: 0,
  });
  const rows = await statement.all();
  assert.deepEqual(rows, [
    { id: 1, name: "Alice" },
    { id: 2, name: "Bob" },
  ]);
});

test("BetterSQLite3Statement pluck() returns first column", async () => {
  const statement = createStatementWithResult({
    rows: [
      [1, "Alice"],
      [2, "Bob"],
    ],
    columns: ["id", "name"],
    changes: 0,
    lastInsertRowId: 0,
  });
  statement.pluck();
  const values = await statement.all();
  assert.deepEqual(values, [1, 2]);
});

test("BetterSQLite3Statement raw() preserves original rows", async () => {
  const originalRows = [
    [1, "Alice"],
    [2, "Bob"],
  ];
  const statement = createStatementWithResult({
    rows: originalRows,
    columns: ["id", "name"],
    changes: 0,
    lastInsertRowId: 0,
  });
  statement.raw();
  const rows = await statement.all();
  assert.deepEqual(rows, originalRows);
});

test("BetterSQLite3Statement expand() nests dotted keys", async () => {
  const statement = createStatementWithResult({
    rows: [
      { "user.id": 1, "user.name": "Alice" },
    ],
    columns: ["user.id", "user.name"],
    changes: 0,
    lastInsertRowId: 0,
  });
  statement.expand();
  const [row] = await statement.all();
  assert.deepEqual(row, {
    user: {
      id: 1,
      name: "Alice",
    },
  });
});

test("BetterSQLite3Statement run() surfaces write metadata", async () => {
  const statement = createStatementWithResult({
    rows: [],
    columns: [],
    changes: 3,
    lastInsertRowId: 42,
  });
  const info = await statement.run();
  assert.equal(info.changes, 3);
  assert.equal(info.lastInsertRowid, 42);
});
