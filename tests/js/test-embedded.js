#!/usr/bin/env node
/**
 * Test script for PluresDB embedded API.
 * Requires native bindings to be built first:
 *   cd crates/pluresdb-node && npm run build
 */

try {
  const { PluresDatabase } = require('./embedded');

  // Test with file-backed database
  const db = new PluresDatabase('test-actor', '/tmp/pluresdb-embedded-test.db');
  console.log('✅ PluresDatabase created');

  // Test SQL operations
  db.exec('CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY, val TEXT)');
  console.log('✅ CREATE TABLE');

  db.exec("INSERT INTO t (val) VALUES ('hello')");
  console.log('✅ INSERT');

  const result = db.query('SELECT * FROM t');
  console.log('✅ SELECT result:', JSON.stringify(result, null, 2));

  // Test CRDT operations
  const nodeId = db.put('test-1', { type: 'greeting', message: 'hello world' });
  console.log('✅ put:', nodeId);

  const node = db.get('test-1');
  console.log('✅ get:', JSON.stringify(node));

  const meta = db.getWithMetadata('test-1');
  console.log('✅ getWithMetadata:', JSON.stringify(meta));

  const searchResults = db.search('hello');
  console.log('✅ search:', JSON.stringify(searchResults));

  const stats = db.stats();
  console.log('✅ stats:', JSON.stringify(stats));

  const actorId = db.getActorId();
  console.log('✅ actorId:', actorId);

  db.delete('test-1');
  console.log('✅ delete');

  console.log('\n🎉 All embedded API tests passed!');
} catch (err) {
  console.error('❌ Test failed:', err.message);
  console.error('\nNote: Native bindings must be built first.');
  console.error('Run: cd crates/pluresdb-node && npm run build');
  process.exit(1);
}
