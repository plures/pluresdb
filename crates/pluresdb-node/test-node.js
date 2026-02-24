// Comprehensive test script for Node.js bindings
// Run with: node test-node.js (after building)

const { PluresDatabase } = require('./index.js');

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
  
  // Test 5: Statistics
  console.log('Test 5: Database statistics');
  const stats = db.stats();
  console.log('  ✓ Stats:', JSON.stringify(stats, null, 2));
  if (stats.totalNodes < 5) {
    throw new Error('Stats failed: incorrect node count');
  }
  console.log('');
  
  // Test 6: Subscriptions
  console.log('Test 6: Subscriptions');
  const subId = db.subscribe();
  console.log('  ✓ Subscribe:', subId);
  console.log('');

  // Test 7: DSL query engine (execDsl / execIr)
  console.log('Test 7: DSL query engine');

  // Seed a fresh database for predictable results
  const qdb = new PluresDatabase('query-actor');
  qdb.put('q1', { category: 'decision', score: 0.9, label: 'A' });
  qdb.put('q2', { category: 'decision', score: 0.5, label: 'B' });
  qdb.put('q3', { category: 'note',     score: 0.7, label: 'C' });
  qdb.put('q4', { category: 'decision', score: 0.8, label: 'D' });

  // 7a: filter + sort + limit via DSL string
  const dslResult = qdb.execDsl('filter(category == "decision") |> sort(by: "score", dir: "desc") |> limit(2)');
  if (!dslResult || !Array.isArray(dslResult.nodes)) {
    throw new Error('execDsl failed: expected { nodes: [...] }, got ' + JSON.stringify(dslResult));
  }
  if (dslResult.nodes.length !== 2) {
    throw new Error('execDsl filter+sort+limit: expected 2 nodes, got ' + dslResult.nodes.length);
  }
  console.log('  ✓ execDsl filter+sort+limit:', dslResult.nodes.length, 'nodes');

  // 7b: aggregate count via DSL string
  const aggResult = qdb.execDsl('aggregate(count)');
  if (!aggResult || aggResult.aggregate === undefined) {
    throw new Error('execDsl aggregate failed: ' + JSON.stringify(aggResult));
  }
  if (aggResult.aggregate !== 4) {
    throw new Error('execDsl aggregate count: expected 4, got ' + aggResult.aggregate);
  }
  console.log('  ✓ execDsl aggregate count:', aggResult.aggregate);

  // 7c: execIr with JSON IR payload
  const irSteps = [
    { op: 'filter', predicate: { field: 'category', cmp: '==', value: 'decision' } },
    { op: 'limit', n: 1 }
  ];
  const irResult = qdb.execIr(irSteps);
  if (!irResult || !Array.isArray(irResult.nodes)) {
    throw new Error('execIr failed: expected { nodes: [...] }, got ' + JSON.stringify(irResult));
  }
  if (irResult.nodes.length !== 1) {
    throw new Error('execIr filter+limit: expected 1 node, got ' + irResult.nodes.length);
  }
  console.log('  ✓ execIr filter+limit:', irResult.nodes.length, 'node');

  // 7d: execDsl with invalid query should throw
  let threw = false;
  try {
    qdb.execDsl('not a valid query!!!');
  } catch (_e) {
    threw = true;
  }
  if (!threw) {
    throw new Error('execDsl invalid query: expected an error to be thrown');
  }
  console.log('  ✓ execDsl invalid query throws');
  console.log('');

  console.log('=== All tests passed! ===');
}

test().catch((error) => {
  console.error('Test failed:', error);
  process.exit(1);
});

