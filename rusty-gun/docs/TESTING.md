# Testing Guide

This document provides comprehensive information about testing the Rusty Gun project.

## Overview

Rusty Gun uses a multi-layered testing approach:

- **Unit Tests**: Test individual components in isolation
- **Integration Tests**: Test component interactions and APIs
- **Performance Tests**: Validate performance characteristics
- **Security Tests**: Ensure security and input validation
- **Benchmarks**: Measure and monitor performance over time

## Test Structure

```
src/tests/
├── unit/                    # Unit tests
│   ├── core.test.ts        # Core database functionality
│   ├── subscriptions.test.ts # Subscription system
│   └── vector-search.test.ts # Vector search functionality
├── integration/            # Integration tests
│   ├── mesh-network.test.ts # P2P networking
│   └── api-server.test.ts  # HTTP/WebSocket API
├── performance/            # Performance tests
│   └── load.test.ts        # Load and stress testing
├── security/               # Security tests
│   └── input-validation.test.ts # Input validation
├── e2e/                    # End-to-end tests
│   └── user-workflows.test.ts # Complete user workflows
└── fixtures/               # Test data and fixtures
    ├── test-data.json      # Sample data for tests
    └── performance-data.json # Performance test data
```

## Running Tests

### All Tests
```bash
deno task test
```

### Specific Test Categories
```bash
# Unit tests only
deno task test:unit

# Integration tests only
deno task test:integration

# Performance tests only
deno task test:performance

# Security tests only
deno task test:security

# End-to-end tests only
deno task test:e2e
```

### Test Coverage
```bash
# Generate coverage report
deno task test:coverage

# View HTML coverage report
deno task test:coverage
# Opens coverage/index.html in browser
```

### Watch Mode
```bash
# Run tests in watch mode
deno task test:watch
```

## Test Categories

### Unit Tests

Unit tests focus on individual components and functions:

- **Core Database**: CRUD operations, persistence, type system
- **Subscriptions**: Event handling, subscription management
- **Vector Search**: Search algorithms, similarity scoring
- **CRDT Operations**: Conflict resolution, vector clocks

Example:
```typescript
Deno.test("Core Database - Basic CRUD Operations", async () => {
  const db = new GunDB();
  // ... test implementation
});
```

### Integration Tests

Integration tests verify component interactions:

- **Mesh Networking**: P2P connections, data synchronization
- **API Server**: HTTP endpoints, WebSocket connections
- **Cross-Component**: Database + Network + API integration

Example:
```typescript
Deno.test("Mesh Network - Basic Connection and Sync", async () => {
  const dbA = new GunDB();
  const dbB = new GunDB();
  // ... test mesh networking
});
```

### Performance Tests

Performance tests validate system performance:

- **Load Testing**: High-volume operations
- **Memory Usage**: Memory consumption patterns
- **Concurrent Operations**: Multi-threaded performance
- **Response Times**: Latency measurements

Example:
```typescript
Deno.test("Performance - Bulk Insert Operations", async () => {
  const count = 1000;
  const metrics = await measureOperation(async () => {
    // ... bulk insert operations
  }, count);
  
  assertEquals(metrics.operationsPerSecond > 100, true);
});
```

### Security Tests

Security tests ensure system security:

- **Input Validation**: SQL injection, XSS prevention
- **Path Traversal**: File system security
- **Memory Exhaustion**: DoS prevention
- **Type Confusion**: Object prototype attacks

Example:
```typescript
Deno.test("Security - SQL Injection Prevention", async () => {
  const maliciousInputs = [
    "'; DROP TABLE users; --",
    "' OR '1'='1"
  ];
  
  for (const input of maliciousInputs) {
    // ... test input validation
  }
});
```

## Benchmarks

### Running Benchmarks

```bash
# Run all benchmarks
deno task benchmark

# Run specific benchmark categories
deno task benchmark:memory
deno task benchmark:network
deno task benchmark:vector
```

### Benchmark Categories

1. **Core Operations**: CRUD performance, vector search speed
2. **Memory Usage**: Memory consumption patterns, leak detection
3. **Network Performance**: Connection handling, data transfer
4. **Vector Search**: Search algorithm performance

### Benchmark Results

Benchmarks generate detailed performance reports:

- Operations per second
- Average response time
- Memory usage patterns
- Performance trends over time

## Test Data

### Fixtures

Test fixtures provide consistent data for testing:

- **test-data.json**: Sample users, documents, projects
- **performance-data.json**: Performance test scenarios
- **security-payloads.json**: Security test inputs

### Test Data Management

```typescript
// Load test data
import testData from "../fixtures/test-data.json";

// Use in tests
const users = testData.users;
const documents = testData.documents;
```

## Continuous Integration

### GitHub Actions

Tests run automatically on:

- Push to main/develop branches
- Pull requests
- Daily scheduled runs

### Test Matrix

Tests run on multiple configurations:

- Deno versions: 1.40.0, 1.41.0
- Node.js versions: 18, 20
- Operating systems: Ubuntu, Windows, macOS

### Coverage Requirements

- Unit test coverage: ≥ 90%
- Integration test coverage: ≥ 80%
- Performance test coverage: All critical paths
- Security test coverage: All attack vectors

## Writing Tests

### Test Structure

```typescript
Deno.test("Test Name", async () => {
  // Arrange
  const db = new GunDB();
  await db.ready(kvPath);
  
  // Act
  const result = await db.someOperation();
  
  // Assert
  assertEquals(result, expectedValue);
  
  // Cleanup
  await db.close();
});
```

### Best Practices

1. **Isolation**: Each test should be independent
2. **Cleanup**: Always clean up resources
3. **Assertions**: Use specific, meaningful assertions
4. **Error Handling**: Test both success and failure cases
5. **Performance**: Keep tests fast and efficient

### Test Utilities

```typescript
// Create temporary database
const kvPath = await Deno.makeTempFile({
  prefix: "kv_",
  suffix: ".sqlite",
});

// Measure performance
const startTime = performance.now();
await operation();
const duration = performance.now() - startTime;

// Generate test data
const testData = generateTestData(1000);
```

## Debugging Tests

### Running Individual Tests

```bash
# Run specific test file
deno test -A src/tests/unit/core.test.ts

# Run specific test by name
deno test -A --filter "Basic CRUD Operations"
```

### Debug Mode

```bash
# Run with debug output
deno test -A --log-level debug

# Run with verbose output
deno test -A --verbose
```

### Test Debugging

```typescript
// Add debug output
console.log("Debug info:", debugData);

// Use assertions for debugging
assertExists(result, "Result should exist");
assertEquals(result.length, expectedLength, "Length mismatch");
```

## Performance Monitoring

### Benchmark Tracking

Benchmarks track performance over time:

- Historical performance data
- Performance regression detection
- Performance improvement trends

### Performance Thresholds

- CRUD operations: ≥ 1000 ops/sec
- Vector search: ≥ 100 ops/sec
- Memory usage: ≤ 5KB per record
- Response time: ≤ 100ms average

### Performance Alerts

Automated alerts for:

- Performance regressions
- Memory leaks
- High error rates
- Slow response times

## Troubleshooting

### Common Issues

1. **Test Timeouts**: Increase timeout or optimize test
2. **Memory Leaks**: Check resource cleanup
3. **Flaky Tests**: Improve test isolation
4. **Slow Tests**: Optimize test data and operations

### Test Environment

```bash
# Check Deno version
deno --version

# Check test environment
deno info

# Verify test configuration
deno task test --dry-run
```

## Contributing

### Adding New Tests

1. Create test file in appropriate category
2. Follow naming conventions
3. Add to test configuration
4. Update documentation

### Test Review Process

1. All tests must pass
2. Code coverage must be maintained
3. Performance tests must meet thresholds
4. Security tests must pass

### Test Maintenance

- Regular test updates
- Performance monitoring
- Security test updates
- Documentation updates

## Resources

- [Deno Testing Documentation](https://deno.land/manual/testing)
- [Test-Driven Development](https://en.wikipedia.org/wiki/Test-driven_development)
- [Performance Testing Best Practices](https://martinfowler.com/articles/practical-test-pyramid.html)
- [Security Testing Guidelines](https://owasp.org/www-project-web-security-testing-guide/)
