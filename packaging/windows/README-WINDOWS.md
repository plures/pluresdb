# PluresDB for Windows - Personal Database

Welcome to PluresDB! Your personal, local-first database for Windows.

## 🎯 What is PluresDB?

PluresDB is a powerful yet simple database designed for personal use on Windows. Think of it as your personal SQLite on steroids with:

- 📝 **Note-taking and Knowledge Management**
- 🔍 **Smart Search** (including AI-powered semantic search)
- 📊 **Data Relationships** (graph database capabilities)
- 🔒 **Privacy First** (all data stays on your computer)
- 🌐 **Beautiful Web UI** (no coding required)
- 💻 **Developer-Friendly** (SQLite-compatible API)

## 🚀 Quick Start (3 Steps)

### Step 1: Start the Server

Double-click `start.bat` or open PowerShell and run:

```powershell
.\pluresdb.exe serve
```

You'll see:

```
✓ PluresDB server started
✓ Web UI: http://localhost:34568
✓ API: http://localhost:34567
```

### Step 2: Open the Web Interface

Open your web browser and go to:

```
http://localhost:34568
```

### Step 3: Create Your First Note

In the Web UI:

1. Click "New Node"
2. Type your note content
3. Click "Save"

That's it! You're ready to use PluresDB.

## 📚 What Can I Do With It?

### Personal Knowledge Base

Store all your notes, documents, and organize your knowledge with powerful search and relationships.

### Task & Project Management

Track your tasks, projects, and to-dos with custom fields and relationships.

### Research Database

Collect articles, papers, and research with automatic metadata and smart search.

### Digital Journal

Maintain a daily journal with tags, mood tracking, and full-text search.

### Bookmark Manager

Save and organize web links with tags, categories, and AI-powered recommendations.

### Contact Database

Store contact information with relationships and custom fields.

### Recipe Collection

Store recipes with searchable ingredients and custom ratings.

## 💡 Common Use Cases

### Example 1: Daily Journal

```json
{
  "type": "journal",
  "date": "2025-11-06",
  "mood": "productive",
  "content": "Today I started using PluresDB...",
  "tags": ["personal", "reflection"]
}
```

### Example 2: Project Tasks

```json
{
  "type": "task",
  "title": "Write documentation",
  "status": "in-progress",
  "priority": "high",
  "project": "PluresDB",
  "dueDate": "2025-11-15"
}
```

### Example 3: Research Notes

```json
{
  "type": "research",
  "title": "Graph Database Paper",
  "source": "https://example.com/paper.pdf",
  "notes": "Interesting approach to...",
  "tags": ["databases", "research"]
}
```

## 🔍 Searching Your Data

### Web UI Search

1. Go to the "Search" tab
2. Type your query
3. Filter by type, date, tags
4. Click any result to view/edit

### Semantic Search

Find notes by meaning, not just keywords:

1. Go to "Vector Search" tab
2. Type what you're looking for
3. PluresDB finds semantically similar content

### CLI Search

```powershell
# Search from command line
.\pluresdb.exe search "my query"

# Semantic search
.\pluresdb.exe vsearch "how to organize notes"
```

## ⚙️ Configuration

### Change Ports

Edit `config.json` or use command line:

```powershell
# Use different ports
.\pluresdb.exe serve --port 8080 --web-port 8081
```

### Change Data Location

```powershell
# Store data in custom location
.\pluresdb.exe serve --data-dir "C:\MyData\PluresDB"
```

### Enable Encryption

```powershell
# Enable encryption for sensitive data
.\pluresdb.exe config set encryption.enabled true
```

## 🛠️ Advanced Features

### Run as Windows Service

```powershell
# Install as service (requires Administrator)
sc.exe create PluresDB binPath= "C:\Program Files\PluresDB\pluresdb.exe serve"
sc.exe start PluresDB
```

### Automatic Backups

Configure in `config.json`:

```json
{
  "autoBackup": true,
  "backupInterval": 3600000,
  "backupDir": "C:\\Users\\YourName\\PluresDB\\Backups"
}
```

### Desktop Shortcut

Create a shortcut to `pluresdb.exe serve` for easy access.

## 💾 Backup & Restore

### Manual Backup

```powershell
# Create backup
.\pluresdb.exe backup --output "C:\Backups\pluresdb-backup.zip"

# Restore from backup
.\pluresdb.exe restore "C:\Backups\pluresdb-backup.zip"
```

### Export Data

```powershell
# Export to JSON
.\pluresdb.exe export --format json --output "my-data.json"

# Export to CSV
.\pluresdb.exe export --format csv --output "my-data.csv"
```

## 🔒 Security & Privacy

- ✅ All data stays on your computer
- ✅ Optional encryption for sensitive data
- ✅ Password protection for web UI
- ✅ No data sent to external servers (unless you enable P2P sync)

### Enable Password Protection

```powershell
# Set password for web UI
.\pluresdb.exe config set auth.enabled true
.\pluresdb.exe config set auth.password "your-secure-password"

# Restart server
.\pluresdb.exe serve
```

## 🆘 Troubleshooting

### Server Won't Start

**Problem**: Port already in use

**Solution**:

```powershell
# Use different port
.\pluresdb.exe serve --port 8080
```

### Cannot Access Web UI

**Problem**: Browser can't connect

**Solution**:

1. Check server is running
2. Try `http://127.0.0.1:34568` instead of `localhost`
3. Check Windows Firewall settings

### Data Not Saving

**Problem**: Changes disappear after restart

**Solution**:

```powershell
# Specify data directory
.\pluresdb.exe serve --data-dir "%USERPROFILE%\.pluresdb\data"
```

### Slow Performance

**Solution**:

```powershell
# Optimize database
.\pluresdb.exe vacuum

# Rebuild indexes
.\pluresdb.exe reindex
```

## 📖 Learn More

### Documentation

- Full documentation: `docs/` folder
- API reference: `docs/API.md`
- Examples: `examples/` folder

### Online Resources

- Website: https://github.com/plures/pluresdb
- Documentation: https://github.com/plures/pluresdb/wiki
- Issues & Support: https://github.com/plures/pluresdb/issues
- Discussions: https://github.com/plures/pluresdb/discussions

## 🎓 Tutorials

### Tutorial 1: Create a Personal Wiki

1. Start PluresDB
2. Create a "concept" type for wiki pages
3. Add pages with links to other pages
4. Use graph view to see connections

### Tutorial 2: Task Management System

1. Create "task", "project", and "milestone" types
2. Link tasks to projects
3. Use tags for priorities
4. Filter by status and due date

### Tutorial 3: Research Database

1. Create "paper", "author", and "topic" types
2. Import papers with metadata
3. Use vector search to find related papers
4. Export citations in various formats

## 🤝 Getting Help

### Quick Help

```powershell
# Show all commands
.\pluresdb.exe --help

# Show command-specific help
.\pluresdb.exe serve --help
.\pluresdb.exe backup --help
```

### Support Channels

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and community help
- **Documentation**: Check the `docs/` folder

## 📝 Tips & Tricks

### Tip 1: Use Tags Liberally

Tags make it easy to find related items later.

### Tip 2: Leverage Vector Search

Use semantic search to find notes even when you don't remember exact keywords.

### Tip 3: Regular Backups

Set up automatic backups to protect your data.

### Tip 4: Customize Your Types

Create custom types that match your workflow.

### Tip 5: Use the Graph View

Visualize relationships between your notes.

## 🌟 Pro Tips

- Use keyboard shortcuts in the Web UI (press `?` to see all shortcuts)
- Set up automatic backups before adding important data
- Use the REST API to integrate with other tools
- Export data regularly for peace of mind
- Start with simple notes, add complexity as needed

## 📋 Checklist for New Users

- [ ] Start the server
- [ ] Open the Web UI
- [ ] Create your first note
- [ ] Try searching
- [ ] Configure a backup location
- [ ] Set a password (if needed)
- [ ] Create some custom types
- [ ] Explore the graph view
- [ ] Try semantic search
- [ ] Read the full documentation

## 🎉 You're Ready!

PluresDB is now ready for you to use. Start by creating some notes and exploring the features. The more you use it, the more powerful it becomes!

---

**Version**: 1.0.1\
**License**: AGPL-3.0\
**Support**: https://github.com/plures/pluresdb/issues

Thank you for using PluresDB! 🚀
