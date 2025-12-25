# PluresDB Node.js Bindings

Native Node.js bindings for PluresDB using N-API.

## Building

```bash
# Install dependencies
npm install

# Build native addon
npm run build

# Build debug version
npm run build:debug
```

## Usage

```javascript
const { PluresDatabase } = require('./index.js');

const db = new PluresDatabase('my-actor-id');

// Put a node
const id = db.put('node-1', { name: 'Test', value: 42 });

// Get a node
const node = db.get('node-1');
console.log(node); // { name: 'Test', value: 42 }

// List all nodes
const all = db.list();
console.log(all);

// Delete a node
db.delete('node-1');
```

## Testing

```bash
# Build first
npm run build

# Run test
node test-node.js
```

## Platform Support

The addon is built for:
- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)
- Windows (x86_64, aarch64)

