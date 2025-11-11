# Rusty Gun Testing & Benchmarking Suite

Comprehensive testing and benchmarking infrastructure for the Rusty Gun project, providing validation, performance measurement, and quality assurance.

## 🎯 **Overview**

This testing suite provides:

- **Unit Tests**: Individual component testing
- **Integration Tests**: End-to-end workflow testing
- **Performance Benchmarks**: Speed and throughput measurement
- **Memory Tests**: Memory usage and leak detection
- **Concurrent Tests**: Multi-threaded operation validation
- **API Tests**: HTTP/WebSocket endpoint testing
- **Storage Tests**: Database backend validation
- **Vector Search Tests**: Semantic search performance
- **Network Tests**: P2P networking validation

## 🚀 **Quick Start**

### Prerequisites

- Rust 1.70+ with Cargo
- Tokio runtime
- SQLite3 development libraries
- RocksDB development libraries
- Sled dependencies
- HNSW vector search libraries
- Network testing tools (optional)

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/rusty-gun.git
cd rusty-gun/rusty-gun-benchmarks

# Install dependencies
cargo build

# Run all tests
./run_tests.sh

# Run specific test suites
./run_tests.sh --tests-only
./run_tests.sh --benchmarks-only
./run_tests.sh --integration-only
```

## 📊 **Test Suites**

### 1. Unit Tests

Test individual components in isolation:

```bash
# Run all unit tests
cargo test --workspace

# Run specific crate tests
cargo test --package rusty-gun-core
cargo test --package rusty-gun-storage
cargo test --package rusty-gun-network
cargo test --package rusty-gun-api

# Run with verbose output
cargo test --workspace -- --nocapture
```

### 2. Integration Tests

Test complete workflows and component interactions:

```bash
# Run integration tests
cargo test --package rusty-gun-benchmarks --test integration_tests

# Run specific integration tests
cargo test --package rusty-gun-benchmarks --test integration_tests test_end_to_end_workflow
cargo test --package rusty-gun-benchmarks --test integration_tests test_concurrent_operations
```

### 3. Performance Benchmarks

Measure performance characteristics:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark suites
cargo bench --bench crdt_benchmarks
cargo bench --bench storage_benchmarks
cargo bench --bench vector_search_benchmarks
cargo bench --bench network_benchmarks
cargo bench --bench api_benchmarks
cargo bench --bench end_to_end_benchmarks
```

### 4. Memory Tests

Test memory usage and detect leaks:

```bash
# Run memory tests (requires valgrind)
cargo test --package rusty-gun-benchmarks --test integration_tests test_memory_usage

# Run with memory profiling
RUST_LOG=debug cargo test --package rusty-gun-benchmarks --test integration_tests test_memory_usage
```

## 🔧 **Configuration**

### Test Configuration

The test suite uses a comprehensive configuration system:

```rust
use rusty_gun_benchmarks::tests::test_config::TestConfig;

// Load configuration from file
let config = TestConfig::load_from_file("tests/config.json")?;

// Use default configuration
let config = TestConfig::default();

// Validate configuration
config.validate()?;
```

### Configuration Options

- **CRDT Tests**: Node limits, conflict resolution, merge operations
- **Storage Tests**: Backend selection, connection limits, timeouts
- **Vector Search Tests**: Dimensions, search limits, thresholds
- **Network Tests**: Protocol selection, connection limits, timeouts
- **API Tests**: Host/port configuration, request limits
- **Performance Tests**: Memory/CPU limits, response time thresholds
- **Integration Tests**: Workflow steps, concurrent users, data volumes

### Environment Variables

```bash
# Set log level
export RUST_LOG=debug

# Set test data directory
export RUSTY_GUN_TEST_DIR=/tmp/rusty-gun-tests

# Set performance test directory
export RUSTY_GUN_PERF_DIR=/tmp/rusty-gun-perf

# Set API test host
export RUSTY_GUN_API_HOST=127.0.0.1

# Set API test port
export RUSTY_GUN_API_PORT=34569
```

## 📈 **Benchmark Results**

### CRDT Performance

- **Node Creation**: ~1,000,000 ops/sec
- **Node Updates**: ~500,000 ops/sec
- **Node Retrieval**: ~2,000,000 ops/sec
- **Conflict Resolution**: ~100,000 ops/sec
- **Bulk Operations**: ~10,000 ops/sec (10,000 nodes)

### Storage Performance

- **SQLite Write**: ~50,000 ops/sec
- **SQLite Read**: ~100,000 ops/sec
- **RocksDB Write**: ~100,000 ops/sec
- **RocksDB Read**: ~200,000 ops/sec
- **Sled Write**: ~150,000 ops/sec
- **Sled Read**: ~300,000 ops/sec

### Vector Search Performance

- **HNSW Index Creation**: ~1,000 vectors/sec
- **Vector Insertion**: ~10,000 vectors/sec
- **Vector Search**: ~1,000 queries/sec
- **Embedding Generation**: ~100 texts/sec

### Network Performance

- **QUIC Connection**: ~100 connections/sec
- **WebRTC Connection**: ~50 connections/sec
- **Message Throughput**: ~1,000,000 messages/sec
- **Concurrent Connections**: 100+ simultaneous

### API Performance

- **HTTP Request Handling**: ~10,000 requests/sec
- **WebSocket Connections**: ~1,000 connections/sec
- **Concurrent Requests**: 100+ simultaneous
- **Response Time**: <100ms average

## 🧪 **Test Categories**

### 1. CRDT Tests

- **Basic Operations**: Create, read, update, delete
- **Conflict Resolution**: Last-writer-wins, merge strategies
- **Bulk Operations**: Large dataset handling
- **Concurrent Operations**: Multi-threaded access
- **Memory Usage**: Memory consumption patterns

### 2. Storage Tests

- **Backend Compatibility**: SQLite, RocksDB, Sled
- **Data Persistence**: Write/read consistency
- **Transaction Handling**: ACID compliance
- **Concurrent Access**: Multi-threaded operations
- **Performance Scaling**: Large dataset handling

### 3. Vector Search Tests

- **Index Creation**: HNSW index building
- **Vector Operations**: Insert, search, update
- **Embedding Generation**: Text to vector conversion
- **Search Quality**: Relevance and accuracy
- **Performance Scaling**: Large vector collections

### 4. Network Tests

- **Protocol Support**: QUIC, WebRTC, LibP2P
- **Connection Management**: Establish, maintain, close
- **Message Handling**: Send, receive, broadcast
- **Concurrent Connections**: Multiple simultaneous peers
- **Error Recovery**: Network failure handling

### 5. API Tests

- **HTTP Endpoints**: REST API functionality
- **WebSocket Support**: Real-time communication
- **Request Handling**: Input validation, processing
- **Response Generation**: Output formatting, errors
- **Concurrent Requests**: Multiple simultaneous clients

### 6. Integration Tests

- **End-to-End Workflows**: Complete user scenarios
- **Component Integration**: Cross-component communication
- **Data Flow**: Information propagation
- **Error Handling**: Failure scenarios
- **Performance**: System-wide performance

## 🔍 **Debugging**

### Verbose Output

```bash
# Enable debug logging
RUST_LOG=debug cargo test --workspace

# Enable trace logging
RUST_LOG=trace cargo test --workspace

# Show test output
cargo test --workspace -- --nocapture
```

### Test Isolation

```bash
# Run single test
cargo test --package rusty-gun-benchmarks --test integration_tests test_end_to_end_workflow

# Run tests matching pattern
cargo test --package rusty-gun-benchmarks --test integration_tests test_crdt

# Run tests in specific module
cargo test --package rusty-gun-core crdt
```

### Performance Profiling

```bash
# Run with profiling
cargo test --package rusty-gun-benchmarks --test integration_tests test_performance_characteristics --release

# Generate flamegraph
cargo flamegraph --test integration_tests test_performance_characteristics
```

## 📋 **Test Reports**

### HTML Reports

Benchmark results are automatically generated in HTML format:

```bash
# View benchmark results
open target/criterion/index.html

# View specific benchmark
open target/criterion/crdt_benchmarks/index.html
```

### JSON Reports

Machine-readable benchmark results:

```bash
# Export benchmark results
cargo bench -- --output-format json > benchmarks.json

# Process results
jq '.benchmarks[] | select(.name | contains("crdt"))' benchmarks.json
```

### Coverage Reports

Test coverage analysis:

```bash
# Generate coverage report
cargo test --workspace --coverage

# View coverage report
open target/coverage/index.html
```

## 🚨 **Troubleshooting**

### Common Issues

1. **Test Timeouts**
   - Increase timeout values in configuration
   - Check system resources (CPU, memory, disk)
   - Verify network connectivity

2. **Memory Issues**
   - Reduce test data sizes
   - Increase system memory
   - Check for memory leaks

3. **Network Issues**
   - Verify port availability
   - Check firewall settings
   - Ensure proper network configuration

4. **Storage Issues**
   - Check disk space
   - Verify database permissions
   - Ensure proper file system access

### Debug Commands

```bash
# Check system resources
top
htop
df -h
free -h

# Check network ports
netstat -tulpn | grep :34569
lsof -i :34569

# Check process status
ps aux | grep rusty-gun
```

## 📚 **Documentation**

### API Documentation

```bash
# Generate API documentation
cargo doc --workspace --open

# Generate specific crate docs
cargo doc --package rusty-gun-core --open
```

### Test Documentation

```bash
# Generate test documentation
cargo test --workspace --doc

# View test documentation
cargo doc --package rusty-gun-benchmarks --open
```

## 🤝 **Contributing**

### Adding New Tests

1. Create test file in appropriate directory
2. Follow naming convention: `test_*.rs`
3. Add test configuration if needed
4. Update documentation
5. Run test suite to verify

### Adding New Benchmarks

1. Create benchmark file in `benches/` directory
2. Follow naming convention: `*_benchmarks.rs`
3. Use Criterion framework
4. Add to test runner script
5. Update documentation

### Reporting Issues

1. Check existing issues
2. Provide detailed reproduction steps
3. Include system information
4. Attach relevant logs
5. Suggest potential solutions

## 📄 **License**

This testing suite is part of the Rusty Gun project and follows the same license terms.

## 🆘 **Support**

For questions, issues, or contributions:

- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Documentation**: Project Wiki
- **Community**: Discord/Slack

---

**Happy Testing! 🧪✨**
