# Publishing Note for pluresdb Crate

## Dependency Configuration

The `pluresdb` crate uses explicit version strings for its dependencies instead of `path` dependencies to ensure successful publishing to crates.io.

### Current Configuration

```toml
[dependencies]
pluresdb-core = "1.5.2"
pluresdb-storage = "1.5.2"
pluresdb-sync = "1.5.2"
```

### Why This Works

1. **For Publishing**: Cargo resolves these versions from crates.io
2. **For Local Development**: Cargo automatically uses workspace members when they exist locally (via workspace resolution)

### Version Synchronization

When updating the workspace version, remember to update these dependency versions in `Cargo.toml` to match.

### Alternative (If Needed)

If you need to use workspace version, you could use:
```toml
[dependencies]
pluresdb-core = { version = "1.5.2" }
```

But explicit version strings are simpler and work reliably for publishing.

