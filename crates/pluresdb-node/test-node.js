// Comprehensive test script for Node.js bindings
// Run with: node test-node.js (after building)

const { PluresDatabase } = require('./index.js');
const fs = require('fs');
const path = require('path');

// Clean up test database
const testDbPath = path.join(__dirname, 'test.db');
if (fs.existsSync(testDbPath)) {
  fs.unlinkSync(testDbPath);
}

async function test() {
  console.log('=== PluresDB Node.js Bindings Test Suite ===\n');
  
  // Test 1: Basic CRUD operations
  console.log('Test 1: Basic CRUD operations');
  const db = new PluresDatabase('test-actor');
  
  console.log('  ✓ Creating database instance');
  console.log('  ✓ Actor ID:', db.getActorId());
  
  // Put
  const id1 = db.put('node-1', { name: 'Alice', age: 30, type: 'Person' });
  console.log('  ✓ Put node:', id1);
  
  // Get
  const node1 = db.get('node-1');
  console.log('  ✓ Get node:', JSON.stringify(node1));
  if (!node1 || node1.name !== 'Alice') {
    throw new Error('Get failed: node data incorrect');
  }
  
  // Get with metadata
  const node1Meta = db.getWithMetadata('node-1');
  console.log('  ✓ Get with metadata:', JSON.stringify(node1Meta, null, 2));
  if (!node1Meta || !node1Meta.clock || !node1Meta.timestamp) {
    throw new Error('Get with metadata failed');
  }
  
  // List
  const all = db.list();
  console.log('  ✓ List nodes:', all.length, 'nodes');
  if (all.length !== 1) {
    throw new Error('List failed: expected 1 node');
  }
  
  // Delete
  db.delete('node-1');
  const deleted = db.get('node-1');
  if (deleted !== null) {
    throw new Error('Delete failed: node still exists');
  }
  console.log('  ✓ Delete node: success\n');
  
  // Test 2: Type filtering
  console.log('Test 2: Type filtering');
  db.put('person-1', { name: 'Bob', type: 'Person' });
  db.put('person-2', { name: 'Charlie', type: 'Person' });
  db.put('item-1', { name: 'Widget', type: 'Item' });
  
  const people = db.listByType('Person');
  console.log('  ✓ List by type "Person":', people.length, 'nodes');
  if (people.length !== 2) {
    throw new Error('List by type failed: expected 2 Person nodes');
  }
  
  const items = db.listByType('Item');
  console.log('  ✓ List by type "Item":', items.length, 'nodes');
  if (items.length !== 1) {
    throw new Error('List by type failed: expected 1 Item node');
  }
  console.log('');
  
  // Test 3: Search
  console.log('Test 3: Text search');
  db.put('doc-1', { title: 'Introduction to Rust', content: 'Rust is a systems programming language' });
  db.put('doc-2', { title: 'JavaScript Guide', content: 'JavaScript is a scripting language' });
  db.put('doc-3', { title: 'Python Tutorial', content: 'Python is a high-level language' });
  
  const rustResults = db.search('Rust', 5);
  console.log('  ✓ Search "Rust":', rustResults.length, 'results');
  if (rustResults.length === 0 || rustResults[0].id !== 'doc-1') {
    throw new Error('Search failed: expected doc-1 in results');
  }
  
  const langResults = db.search('language', 10);
  console.log('  ✓ Search "language":', langResults.length, 'results');
  if (langResults.length < 3) {
    throw new Error('Search failed: expected at least 3 results');
  }
  console.log('');
  
  // Test 4: Vector search
  console.log('Test 4: Vector search');
  const dim = 4;
  const embRust = [1.0, 0.0, 0.0, 0.0];
  const embJs   = [0.0, 1.0, 0.0, 0.0];
  const embPy   = [0.0, 0.0, 1.0, 0.0];

  db.putWithEmbedding('emb-rust', { title: 'Rust' }, embRust);
  db.putWithEmbedding('emb-js',   { title: 'JavaScript' }, embJs);
  db.putWithEmbedding('emb-py',   { title: 'Python' }, embPy);

  const vectorResults = db.vectorSearch(embRust, 3, 0.0);
  console.log('  ✓ Vector search results:', vectorResults.length);
  if (vectorResults.length === 0 || vectorResults[0].id !== 'emb-rust') {
    throw new Error('Vector search failed: expected emb-rust as top result, got: ' + JSON.stringify(vectorResults));
  }
  if (vectorResults[0].score < 0.99) {
    throw new Error('Vector search failed: expected score ~1.0 for identical vector, got: ' + vectorResults[0].score);
  }
  console.log('  ✓ Top result:', vectorResults[0].id, 'score:', vectorResults[0].score);
  console.log('');
  
  // Test 5: SQL queries (requires database)
  console.log('Test 5: SQL queries');
  const dbWithSql = new PluresDatabase('test-actor', testDbPath);
  
  // Create table
  dbWithSql.exec(`
    CREATE TABLE IF NOT EXISTS users (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      name TEXT NOT NULL,
      email TEXT UNIQUE
    )
  `);
  console.log('  ✓ Created table');
  
  // Insert data
  dbWithSql.exec(`INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')`);
  dbWithSql.exec(`INSERT INTO users (name, email) VALUES ('Bob', 'bob@example.com')`);
  console.log('  ✓ Inserted data');
  
  // Query data
  const queryResult = dbWithSql.query('SELECT * FROM users WHERE name = ?', ['Alice']);
  console.log('  ✓ Query result:', JSON.stringify(queryResult, null, 2));
  if (queryResult.rows.length !== 1 || queryResult.rows[0].name !== 'Alice') {
    throw new Error('Query failed: incorrect results');
  }
  
  // Clean up
  if (fs.existsSync(testDbPath)) {
    fs.unlinkSync(testDbPath);
  }
  console.log('');
  
  // Test 6: Statistics
  console.log('Test 6: Database statistics');
  const stats = db.stats();
  console.log('  ✓ Stats:', JSON.stringify(stats, null, 2));
  if (stats.totalNodes < 5) {
    throw new Error('Stats failed: incorrect node count');
  }
  console.log('');
  
  // Test 7: Subscriptions
  console.log('Test 7: Subscriptions');
  const subId = db.subscribe();
  console.log('  ✓ Subscribe:', subId);
  console.log('');
  
  console.log('=== All tests passed! ===');
}

test().catch((error) => {
  console.error('Test failed:', error);
  process.exit(1);
});

