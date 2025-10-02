// Rusty Gun - Local-First Database with P2P Capabilities
// Main entry point with SQLite compatibility

export { RustyGunNode } from './main.ts';
export { SQLiteCompatibleAPI } from './sqlite-compat.ts';

// SQLite compatibility exports
export { open, Database } from './sqlite-compat.ts';
export { default as sqlite3, Database as SQLite3Database } from './sqlite3-compat.ts';

// Re-export for backward compatibility
export { RustyGunNode as default } from './main.ts';