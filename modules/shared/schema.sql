-- PluresDB Command History Schema
-- This schema supports command history tracking across multiple shells and hosts

-- Command history table with comprehensive tracking
CREATE TABLE IF NOT EXISTS command_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    command TEXT NOT NULL,
    hostname TEXT NOT NULL,
    shell_type TEXT NOT NULL, -- powershell, bash, zsh, fish, etc.
    shell_version TEXT,
    username TEXT,
    working_directory TEXT,
    timestamp INTEGER NOT NULL, -- Unix timestamp in milliseconds
    duration_ms INTEGER, -- Command execution duration
    exit_code INTEGER, -- Command exit status
    output TEXT, -- Command output (optional, can be large)
    error_output TEXT, -- Error output (optional)
    session_id TEXT, -- Shell session identifier
    environment_vars TEXT, -- JSON blob of relevant env vars
    is_success BOOLEAN GENERATED ALWAYS AS (exit_code = 0) STORED,
    sync_timestamp INTEGER DEFAULT (unixepoch() * 1000), -- For P2P sync
    device_id TEXT, -- Device/peer identifier for sync
    UNIQUE(hostname, shell_type, command, timestamp) ON CONFLICT IGNORE
);

-- Index for fast lookups by hostname and shell
CREATE INDEX IF NOT EXISTS idx_command_history_host_shell 
    ON command_history(hostname, shell_type);

-- Index for timestamp-based queries
CREATE INDEX IF NOT EXISTS idx_command_history_timestamp 
    ON command_history(timestamp DESC);

-- Index for success/failure queries
CREATE INDEX IF NOT EXISTS idx_command_history_status 
    ON command_history(is_success, exit_code);

-- Index for command search
CREATE INDEX IF NOT EXISTS idx_command_history_command 
    ON command_history(command);

-- Index for session-based queries
CREATE INDEX IF NOT EXISTS idx_command_history_session 
    ON command_history(session_id, timestamp);

-- Deduplicated command view - shows unique commands with last execution
CREATE VIEW IF NOT EXISTS command_history_unique AS
SELECT 
    command,
    hostname,
    shell_type,
    MAX(timestamp) as last_executed,
    COUNT(*) as execution_count,
    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success_count,
    SUM(CASE WHEN is_success THEN 0 ELSE 1 END) as failure_count,
    AVG(duration_ms) as avg_duration_ms,
    GROUP_CONCAT(DISTINCT username) as users
FROM command_history
GROUP BY command, hostname, shell_type;

-- Command frequency view - most commonly used commands
CREATE VIEW IF NOT EXISTS command_frequency AS
SELECT 
    command,
    COUNT(*) as total_executions,
    COUNT(DISTINCT hostname) as unique_hosts,
    COUNT(DISTINCT shell_type) as unique_shells,
    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success_count,
    SUM(CASE WHEN is_success THEN 0 ELSE 1 END) as failure_count,
    AVG(duration_ms) as avg_duration_ms,
    MIN(timestamp) as first_seen,
    MAX(timestamp) as last_seen
FROM command_history
GROUP BY command
ORDER BY total_executions DESC;

-- Failed commands view - for troubleshooting
CREATE VIEW IF NOT EXISTS failed_commands AS
SELECT 
    command,
    hostname,
    shell_type,
    timestamp,
    exit_code,
    error_output,
    working_directory,
    duration_ms
FROM command_history
WHERE is_success = 0
ORDER BY timestamp DESC;

-- Session history view - commands grouped by session
CREATE VIEW IF NOT EXISTS session_history AS
SELECT 
    session_id,
    hostname,
    shell_type,
    MIN(timestamp) as session_start,
    MAX(timestamp) as session_end,
    COUNT(*) as command_count,
    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success_count,
    SUM(CASE WHEN is_success THEN 0 ELSE 1 END) as failure_count
FROM command_history
WHERE session_id IS NOT NULL
GROUP BY session_id
ORDER BY session_start DESC;

-- Host summary view - statistics per host
CREATE VIEW IF NOT EXISTS host_summary AS
SELECT 
    hostname,
    COUNT(DISTINCT shell_type) as shell_types,
    COUNT(*) as total_commands,
    SUM(CASE WHEN is_success THEN 1 ELSE 0 END) as success_count,
    SUM(CASE WHEN is_success THEN 0 ELSE 1 END) as failure_count,
    MIN(timestamp) as first_activity,
    MAX(timestamp) as last_activity,
    COUNT(DISTINCT DATE(timestamp / 1000, 'unixepoch')) as active_days
FROM command_history
GROUP BY hostname;

-- Configuration table for module settings
CREATE TABLE IF NOT EXISTS command_history_config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    updated_at INTEGER DEFAULT (unixepoch() * 1000)
);

-- Default configuration values
INSERT OR IGNORE INTO command_history_config (key, value, description) VALUES
    ('capture_output', 'false', 'Whether to capture command output (can be large)'),
    ('capture_env', 'false', 'Whether to capture environment variables'),
    ('max_output_size', '10240', 'Maximum output size in bytes to capture'),
    ('dedup_window_ms', '1000', 'Time window in ms for deduplicating identical commands'),
    ('auto_cleanup_days', '90', 'Days to keep command history (0 = never cleanup)'),
    ('sync_enabled', 'true', 'Enable P2P sync of command history'),
    ('ignore_patterns', '', 'Comma-separated patterns to ignore (e.g., ls,cd,pwd)');
