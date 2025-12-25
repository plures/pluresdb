// Simple test script for Node.js bindings
// Run with: node test-node.js (after building)

const { PluresDatabase } = require('./index.js');

async function test() {
  console.log('Creating database instance...');
  const db = new PluresDatabase('test-actor');
  
  console.log('Testing put...');
  const id = db.put('test-node-1', { name: 'Test Node', value: 42 });
  console.log('Put result:', id);
  
  console.log('Testing get...');
  const result = db.get('test-node-1');
  console.log('Get result:', result);
  
  console.log('Testing list...');
  const all = db.list();
  console.log('List result:', JSON.stringify(all, null, 2));
  
  console.log('Testing delete...');
  db.delete('test-node-1');
  console.log('Delete completed');
  
  console.log('Testing get after delete...');
  const deleted = db.get('test-node-1');
  console.log('Get after delete:', deleted);
  
  console.log('All tests completed!');
}

test().catch(console.error);

