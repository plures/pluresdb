# Rusty Gun VSCode Extension

A comprehensive VSCode extension for Rusty Gun graph database with vector search capabilities.

## Features

### 🗄️ **Database Management**
- **Node Operations**: Create, read, update, delete nodes
- **Relationship Management**: Create and manage relationships between nodes
- **SQL Query Support**: Execute SQL queries with syntax highlighting
- **Graph Visualization**: Interactive graph view with Cytoscape.js
- **Real-time Updates**: Live data synchronization via WebSocket

### 🔍 **Vector Search**
- **Semantic Search**: Search using natural language queries
- **Similarity Search**: Find similar content based on vector embeddings
- **Configurable Thresholds**: Adjustable similarity thresholds
- **Multiple Models**: Support for various embedding models

### 📊 **Analytics & Monitoring**
- **System Statistics**: Real-time database statistics
- **Performance Metrics**: Monitor query performance and system health
- **Service Status**: Health monitoring for all services
- **Usage Analytics**: Track vector search and database usage

### 🎨 **User Interface**
- **Explorer View**: Tree view of nodes and relationships
- **Dashboard**: Real-time system overview
- **Graph View**: Interactive graph visualization
- **Query Results**: Formatted query result display
- **Status Bar**: Connection status and quick actions

## Installation

### From VSIX Package
1. Download the latest `.vsix` file from releases
2. Open VSCode
3. Go to Extensions (Ctrl+Shift+X)
4. Click the "..." menu and select "Install from VSIX..."
5. Select the downloaded `.vsix` file

### From Source
1. Clone the repository
2. Run `npm install`
3. Run `npm run compile`
4. Press F5 to open a new Extension Development Host window

## Configuration

### Server Settings
- **Server URL**: Configure the Rusty Gun server endpoint (default: `http://localhost:34569`)
- **Auto Connect**: Automatically connect on startup
- **Vector Search Threshold**: Default similarity threshold (0.0-1.0)
- **Max Results**: Maximum number of results to display
- **Notifications**: Enable/disable notifications
- **Theme**: Light, dark, or auto theme selection

### Usage

#### 1. Connect to Server
- Use the command palette (Ctrl+Shift+P)
- Run "Rusty Gun: Connect to Rusty Gun"
- Or click the connection status in the status bar

#### 2. Explore Database
- Open the "Rusty Gun Database" view in the Explorer
- Browse nodes and relationships
- View node details and metadata

#### 3. Execute Queries
- Open a SQL file or create a new one
- Select the query text
- Right-click and select "Execute SQL Query"
- Or use the command palette

#### 4. Vector Search
- Use the "Vector Search" view
- Enter your search query
- Adjust threshold and result limits
- View similarity scores and metadata

#### 5. Graph Visualization
- Open the "Graph View" in the Rusty Gun panel
- Use zoom, pan, and layout controls
- Click nodes to view details
- Toggle between different layouts

## Commands

### Database Commands
- `rusty-gun.connect` - Connect to Rusty Gun server
- `rusty-gun.disconnect` - Disconnect from server
- `rusty-gun.openDashboard` - Open the dashboard
- `rusty-gun.executeQuery` - Execute SQL query
- `rusty-gun.refreshExplorer` - Refresh explorer view

### Node Commands
- `rusty-gun.createNode` - Create a new node
- `rusty-gun.editNode` - Edit existing node
- `rusty-gun.deleteNode` - Delete node
- `rusty-gun.viewNode` - View node details
- `rusty-gun.copyNodeId` - Copy node ID to clipboard

### Relationship Commands
- `rusty-gun.createRelationship` - Create relationship
- `rusty-gun.editRelationship` - Edit relationship
- `rusty-gun.deleteRelationship` - Delete relationship
- `rusty-gun.viewRelationship` - View relationship details

### Vector Search Commands
- `rusty-gun.vectorSearch` - Perform vector search
- `rusty-gun.searchSimilar` - Search for similar content

### Graph Commands
- `rusty-gun.zoomToFit` - Zoom to fit all nodes
- `rusty-gun.centerGraph` - Center the graph view
- `rusty-gun.toggleLayout` - Toggle graph layout

### Export/Import Commands
- `rusty-gun.exportGraph` - Export graph data
- `rusty-gun.importGraph` - Import graph data

## Views

### Explorer View
- **Nodes**: List of all nodes in the database
- **Relationships**: List of all relationships
- **Graph Stats**: Quick access to graph statistics

### Vector Search View
- **Search Interface**: Query input and results
- **Similarity Results**: Ranked search results
- **Metadata Display**: Detailed result information

### Graph View
- **Interactive Graph**: Cytoscape.js visualization
- **Node Details**: Click to view node information
- **Layout Controls**: Different graph layouts
- **Zoom Controls**: Pan and zoom functionality

### Analytics View
- **Server Status**: System health monitoring
- **Graph Statistics**: Database metrics
- **Vector Statistics**: Search performance metrics
- **Service Health**: Individual service status

## Language Support

### Rusty Gun Query Language
- **File Extension**: `.rgql`, `.rusty-gun`
- **Syntax Highlighting**: Custom language support
- **Snippets**: Code completion and templates
- **IntelliSense**: Smart code suggestions

### SQL Support
- **Enhanced SQL**: Extended SQL with graph operations
- **Snippets**: Rusty Gun specific SQL templates
- **Syntax Highlighting**: Full SQL support
- **Query Execution**: Direct execution from editor

## Snippets

### Rusty Gun Query Snippets
- `create-node` - Create a new node
- `create-rel` - Create a relationship
- `vector-search` - Perform vector search
- `sql` - SQL query template
- `traverse` - Graph traversal query

### SQL Snippets
- `rg-select` - SELECT query
- `rg-insert-node` - INSERT node
- `rg-insert-rel` - INSERT relationship
- `rg-update-node` - UPDATE node
- `rg-delete-node` - DELETE node
- `rg-join` - JOIN query

## Development

### Prerequisites
- Node.js 18+
- TypeScript 5+
- VSCode 1.85+

### Building
```bash
npm install
npm run compile
npm run watch  # For development
```

### Testing
```bash
npm run test
npm run lint
```

### Packaging
```bash
npm run package
```

## Architecture

### Core Components
- **RustyGunClient**: API client for server communication
- **Tree Providers**: Data providers for tree views
- **Webview Providers**: Dashboard and visualization
- **Command Handlers**: Command execution logic
- **Configuration Manager**: Settings management

### Data Flow
1. **User Action** → Command/View
2. **Command Handler** → RustyGunClient
3. **API Request** → Rusty Gun Server
4. **Response** → Tree Provider/Webview
5. **UI Update** → User Interface

### State Management
- **Connection State**: Server connection status
- **Data Cache**: Local data caching
- **Configuration**: User preferences
- **Notifications**: User feedback system

## Troubleshooting

### Common Issues
1. **Connection Failed**: Check server URL and ensure server is running
2. **Query Errors**: Verify SQL syntax and table names
3. **Vector Search Issues**: Check embedding model configuration
4. **Graph View Problems**: Ensure Cytoscape.js is loaded

### Debug Mode
1. Open Command Palette (Ctrl+Shift+P)
2. Run "Developer: Toggle Developer Tools"
3. Check console for error messages
4. Use "Rusty Gun: Show Output" for detailed logs

### Logs
- **Output Channel**: "Rusty Gun" in Output panel
- **Developer Console**: Browser developer tools
- **Extension Logs**: VSCode extension host logs

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## License

MIT License - see LICENSE file for details

## Support

- **Issues**: GitHub Issues
- **Documentation**: GitHub Wiki
- **Discussions**: GitHub Discussions
- **Email**: support@rusty-gun.dev

## Changelog

### v0.1.0
- Initial release
- Basic database operations
- Vector search support
- Graph visualization
- SQL query execution
- Real-time updates
- Analytics dashboard


