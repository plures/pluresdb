# pluresdb-cli

Command-line interface for managing PluresDB nodes.

## Features

- **Database Management**
  - Initialize databases
  - Start API server
  - Database status and statistics

- **CRUD Operations**
  - Insert/update nodes (`put`)
  - Retrieve nodes (`get`)
  - Delete nodes (`delete`)
  - List nodes (`list`)

- **SQL Support**
  - Execute SQL queries (`query`)
  - Execute SQL statements (`exec`)

- **Search**
  - Full-text search (`search`)
  - Vector similarity search (`vsearch`)

- **Type System**
  - Define types (`type define`)
  - List types (`type list`)
  - Get type instances (`type instances`)
  - Show type schema (`type schema`)

- **Network**
  - Connect to peers (`network connect`)
  - List peers (`network peers`)
  - Synchronize (`network sync`)

- **Configuration**
  - List configuration (`config list`)
  - Get/set configuration (`config get/set`)
  - Reset configuration (`config reset`)

- **Maintenance**
  - Backup/restore (`maintenance backup/restore`)
  - Database vacuum (`maintenance vacuum`)
  - Run migrations (`maintenance migrate`)
  - Show statistics (`maintenance stats`)

- **API Server**
  - HTTP REST API
  - WebSocket support
  - CORS enabled

## Installation

### From crates.io

```bash
cargo install pluresdb-cli
```

### From Source

```bash
git clone https://github.com/plures/pluresdb.git
cd pluresdb
cargo build --release --bin pluresdb
```

## Usage

### Initialize Database

```bash
pluresdb init ./my-database
```

### Start API Server

```bash
pluresdb serve --port 34569
```

### CRUD Operations

```bash
# Insert a node
pluresdb put node-1 '{"name": "Alice", "age": 30}' --actor my-actor

# Get a node
pluresdb get node-1

# List all nodes
pluresdb list

# Delete a node
pluresdb delete node-1
```

### SQL Queries

```bash
# Query with parameters
pluresdb query "SELECT * FROM users WHERE age > ?" --params '[25]'

# Execute statement
pluresdb exec "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)"
```

### Search

```bash
# Text search
pluresdb search "Alice" --limit 10

# Vector search
pluresdb vsearch "machine learning" --limit 5 --threshold 0.7
```

### Type System

```bash
# Define a type
pluresdb type define Person '{"properties": {"name": {"type": "string"}}}'

# List types
pluresdb type list

# Get instances
pluresdb type instances Person
```

### Network

```bash
# Connect to peer
pluresdb network connect ws://localhost:34569

# List peers
pluresdb network peers

# Sync
pluresdb network sync
```

### Maintenance

```bash
# Backup
pluresdb maintenance backup ./backup.json --compress

# Restore
pluresdb maintenance restore ./backup.json

# Vacuum
pluresdb maintenance vacuum --stats

# Statistics
pluresdb maintenance stats --detailed
```

## Configuration

Configuration is stored in `config.json` in the data directory:

```json
{
  "log_level": "info",
  "max_connections": 100,
  "enable_websocket": true
}
```

## API Server

The API server provides REST endpoints:

- `GET /health` - Health check
- `GET /api/nodes` - List nodes
- `POST /api/nodes` - Create node
- `GET /api/nodes/:id` - Get node
- `DELETE /api/nodes/:id` - Delete node

WebSocket support is available at `/ws` for real-time updates.

## Examples

See the `examples/` directory for more usage examples.

## License

AGPL-3.0

