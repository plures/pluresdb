# Getting Started with PluresDB on Windows

**PluresDB** is a powerful P2P graph database with SQLite compatibility, perfect for personal note-taking, knowledge management, and data storage on Windows.

## 🚀 Quick Start

### Installation Options

#### Option 1: Using winget (Recommended)

```powershell
# Install PluresDB via Windows Package Manager
winget install plures.pluresdb
```

#### Option 2: Using PowerShell Install Script

```powershell
# Download and run the installer
irm https://raw.githubusercontent.com/plures/pluresdb/main/install.ps1 | iex
```

#### Option 3: Manual Installation from ZIP

1. Download the latest release from [GitHub Releases](https://github.com/plures/pluresdb/releases)
2. Extract `pluresdb-windows-x64.zip` to your preferred location
3. Run `install.bat` from the extracted folder

#### Option 4: Using MSI Installer

1. Download `pluresdb.msi` from [GitHub Releases](https://github.com/plures/pluresdb/releases)
2. Double-click to install
3. Follow the installation wizard

## 📝 Personal Database Use Cases

PluresDB is ideal for:

- **Personal Knowledge Base**: Store notes, documents, and organize your thoughts
- **Research Database**: Collect research papers, articles, and maintain relationships
- **Project Management**: Track projects, tasks, and dependencies
- **Journal/Diary**: Maintain daily logs with rich metadata
- **Bookmark Manager**: Save and organize web links with tags
- **Recipe Collection**: Store recipes with searchable ingredients
- **Password Vault**: Encrypted storage of sensitive information
- **Contact Database**: Manage contacts with rich relationships

## 🎯 Your First Database

### 1. Start the Database Server

Open PowerShell or Command Prompt:

```powershell
# Start the database server
pluresdb serve

# Or start with custom settings
pluresdb serve --port 34567 --data-dir C:\MyData\pluresdb
```

You'll see:
```
PluresDB server starting...
✓ API server listening on http://localhost:34567
✓ Web UI available at http://localhost:34568
✓ Data directory: C:\Users\YourName\.pluresdb
```

### 2. Access the Web UI

Open your web browser and navigate to:
```
http://localhost:34568
```

The web UI provides:
- **Data Explorer**: Browse and edit your data visually
- **Graph View**: See relationships between your notes
- **Search**: Full-text and semantic search across all data
- **Import/Export**: Backup and restore your database
- **Settings**: Configure the database to your needs

### 3. Create Your First Note

Using the Web UI:
1. Click "New Node" in the explorer
2. Add content:
   ```json
   {
     "type": "note",
     "title": "My First Note",
     "content": "Getting started with PluresDB!",
     "tags": ["personal", "getting-started"],
     "created": "2025-11-06"
   }
   ```
3. Click "Save"

Using the CLI:
```powershell
# Create a new note
pluresdb put note:1 '{
  "type": "note",
  "title": "My First Note",
  "content": "Getting started with PluresDB!"
}'

# Retrieve the note
pluresdb get note:1

# Search for notes
pluresdb search "getting started"
```

## 💡 Common Personal Database Patterns

### Note-Taking System

```json
{
  "type": "note",
  "title": "Meeting Notes",
  "content": "Discussion about project timeline...",
  "tags": ["work", "meetings", "project-alpha"],
  "date": "2025-11-06",
  "linkedNotes": ["note:123", "note:456"]
}
```

### Task Management

```json
{
  "type": "task",
  "title": "Complete documentation",
  "status": "in-progress",
  "priority": "high",
  "dueDate": "2025-11-15",
  "project": "project:alpha",
  "tags": ["documentation"]
}
```

### Knowledge Graph

```json
{
  "type": "concept",
  "name": "PluresDB",
  "description": "P2P graph database",
  "relatedTo": ["concept:database", "concept:p2p"],
  "resources": ["https://github.com/plures/pluresdb"]
}
```

## 🔧 Configuration for Personal Use

Create a configuration file at `%USERPROFILE%\.pluresdb\config.json`:

```json
{
  "port": 34567,
  "webPort": 34568,
  "dataDir": "C:\\Users\\YourName\\.pluresdb\\data",
  "autoBackup": true,
  "backupInterval": 3600000,
  "enableEncryption": true,
  "features": {
    "vectorSearch": true,
    "autoEmbedding": true,
    "p2pSync": false
  }
}
```

## 📊 Organizing Your Data

### Using Types

PluresDB supports custom types to organize your data:

```powershell
# Define a new type
pluresdb define-type book '{
  "name": "book",
  "schema": {
    "properties": {
      "title": { "type": "string" },
      "author": { "type": "string" },
      "isbn": { "type": "string" },
      "rating": { "type": "number" }
    },
    "required": ["title", "author"]
  }
}'

# Create instances
pluresdb put book:1 '{
  "title": "Database Systems",
  "author": "Ramakrishnan",
  "isbn": "978-0-07-246563-4",
  "rating": 4.5
}'
```

### Using Tags

```powershell
# Query by tag
pluresdb search --tag "work"
pluresdb search --tag "personal"

# Multiple tags
pluresdb search --tag "project-alpha" --tag "high-priority"
```

## 🔍 Searching Your Data

### Full-Text Search

```powershell
# Simple search
pluresdb search "meeting notes"

# Search with filters
pluresdb search "database" --type note

# Date range search
pluresdb search --after 2025-11-01 --before 2025-11-30
```

### Vector Search (Semantic)

PluresDB includes AI-powered semantic search:

```powershell
# Find semantically similar content
pluresdb vsearch "how to organize personal notes"

# Find similar to a specific node
pluresdb vsearch --similar-to note:123
```

## 💾 Backup and Restore

### Automatic Backups

Configure automatic backups in your config:

```json
{
  "autoBackup": true,
  "backupInterval": 3600000,
  "backupDir": "C:\\Users\\YourName\\.pluresdb\\backups",
  "maxBackups": 10
}
```

### Manual Backup

```powershell
# Create a backup
pluresdb backup --output C:\Backups\pluresdb-backup.zip

# Restore from backup
pluresdb restore C:\Backups\pluresdb-backup.zip
```

### Export Data

```powershell
# Export to JSON
pluresdb export --format json --output data.json

# Export to CSV
pluresdb export --format csv --output data.csv

# Export specific type
pluresdb export --type note --output notes.json
```

## 🌐 Accessing from Code

### Using Node.js

```javascript
import { PluresNode, SQLiteCompatibleAPI } from "pluresdb";

// Start the database
const db = new PluresNode({
  config: {
    port: 34567,
    dataDir: "./data",
  },
  autoStart: true,
});

// Use SQLite-compatible API
const sqlite = new SQLiteCompatibleAPI();

// Create a note
await sqlite.run(
  "INSERT INTO nodes (id, data) VALUES (?, ?)",
  ["note:1", JSON.stringify({ title: "My Note" })]
);

// Query notes
const notes = await sqlite.all(
  "SELECT * FROM nodes WHERE json_extract(data, '$.type') = 'note'"
);
```

### Using Python (via REST API)

```python
import requests
import json

# Base URL
base_url = "http://localhost:34567/api"

# Create a note
note = {
    "type": "note",
    "title": "Python Note",
    "content": "Created from Python"
}
response = requests.put(f"{base_url}/nodes/note:1", json=note)

# Get a note
response = requests.get(f"{base_url}/nodes/note:1")
note = response.json()

# Search notes
response = requests.get(f"{base_url}/search?q=python")
results = response.json()
```

## 🔒 Security Best Practices

### Enable Encryption

```json
{
  "encryption": {
    "enabled": true,
    "algorithm": "AES-256-GCM",
    "keyDerivation": "PBKDF2"
  }
}
```

### Set Access Password

```powershell
# Set password for web UI
pluresdb config set auth.enabled true
pluresdb config set auth.password "your-secure-password"

# Restart server
pluresdb serve
```

### Backup Encryption Keys

```powershell
# Export encryption key (keep safe!)
pluresdb config get encryption.key > encryption-key.txt

# Backup to secure location
copy encryption-key.txt D:\SecureBackup\
```

## 🚀 Advanced Features

### Running as Windows Service

```powershell
# Install as service (run as Administrator)
sc.exe create PluresDB binPath= "C:\Program Files\PluresDB\pluresdb.exe serve"
sc.exe start PluresDB

# Configure service to auto-start
sc.exe config PluresDB start= auto
```

### Desktop Shortcut

The MSI installer creates a desktop shortcut automatically. For manual setup:

1. Right-click on Desktop → New → Shortcut
2. Location: `C:\Program Files\PluresDB\pluresdb.exe serve`
3. Name: "PluresDB"
4. Click Finish

### Browser Integration

Add to browser favorites:
- **Web UI**: http://localhost:34568
- **API Docs**: http://localhost:34567/docs

## 📚 Next Steps

1. **Explore the Web UI**: http://localhost:34568
2. **Read the API Documentation**: `/docs/API.md`
3. **Check out Examples**: `/examples/`
4. **Join the Community**: [GitHub Discussions](https://github.com/plures/pluresdb/discussions)

## 🆘 Troubleshooting

### Server Won't Start

```powershell
# Check if port is in use
netstat -ano | findstr :34567

# Use different port
pluresdb serve --port 8080
```

### Cannot Access Web UI

1. Check firewall settings
2. Try `http://127.0.0.1:34568` instead of `localhost`
3. Ensure server is running: `pluresdb status`

### Data Not Persisting

1. Check data directory exists: `echo %USERPROFILE%\.pluresdb`
2. Verify write permissions
3. Check disk space: `wmic logicaldisk get size,freespace,caption`

### Performance Issues

```powershell
# Check database stats
pluresdb stats

# Optimize database
pluresdb vacuum

# Rebuild indexes
pluresdb reindex
```

## 🤝 Getting Help

- **Documentation**: [GitHub Wiki](https://github.com/plures/pluresdb/wiki)
- **Issues**: [Report bugs](https://github.com/plures/pluresdb/issues)
- **Discussions**: [Ask questions](https://github.com/plures/pluresdb/discussions)
- **Email**: support@pluresdb.io

---

**Happy note-taking and organizing! 🎉**
