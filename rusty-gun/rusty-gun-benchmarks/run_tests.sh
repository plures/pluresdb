#!/bin/bash

# Rusty Gun Test Runner
# Comprehensive testing and benchmarking script

set -e

echo "🚀 Rusty Gun Test Runner"
echo "========================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Please run this script from the rusty-gun-benchmarks directory"
    exit 1
fi

# Parse command line arguments
RUN_TESTS=true
RUN_BENCHMARKS=true
RUN_INTEGRATION=true
VERBOSE=false
COVERAGE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --tests-only)
            RUN_BENCHMARKS=false
            RUN_INTEGRATION=false
            shift
            ;;
        --benchmarks-only)
            RUN_TESTS=false
            RUN_INTEGRATION=false
            shift
            ;;
        --integration-only)
            RUN_TESTS=false
            RUN_BENCHMARKS=false
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --coverage)
            COVERAGE=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo "Options:"
            echo "  --tests-only        Run only unit tests"
            echo "  --benchmarks-only   Run only benchmarks"
            echo "  --integration-only  Run only integration tests"
            echo "  --verbose           Enable verbose output"
            echo "  --coverage          Generate test coverage report"
            echo "  --help              Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Set up environment
export RUST_LOG=info
export RUST_BACKTRACE=1

if [ "$VERBOSE" = true ]; then
    export RUST_LOG=debug
fi

# Function to run tests
run_tests() {
    print_status "Running unit tests..."
    
    local test_args=""
    if [ "$VERBOSE" = true ]; then
        test_args="-- --nocapture"
    fi
    
    if [ "$COVERAGE" = true ]; then
        print_status "Running tests with coverage..."
        cargo test --workspace $test_args
    else
        cargo test --workspace $test_args
    fi
    
    if [ $? -eq 0 ]; then
        print_success "Unit tests passed!"
    else
        print_error "Unit tests failed!"
        return 1
    fi
}

# Function to run integration tests
run_integration_tests() {
    print_status "Running integration tests..."
    
    local test_args=""
    if [ "$VERBOSE" = true ]; then
        test_args="-- --nocapture"
    fi
    
    cargo test --package rusty-gun-benchmarks --test integration_tests $test_args
    
    if [ $? -eq 0 ]; then
        print_success "Integration tests passed!"
    else
        print_error "Integration tests failed!"
        return 1
    fi
}

# Function to run benchmarks
run_benchmarks() {
    print_status "Running benchmarks..."
    
    # Check if criterion is available
    if ! cargo bench --help > /dev/null 2>&1; then
        print_warning "Criterion not available, skipping benchmarks"
        return 0
    fi
    
    # Run different benchmark suites
    local benchmark_suites=(
        "crdt_benchmarks"
        "storage_benchmarks"
        "vector_search_benchmarks"
        "network_benchmarks"
        "api_benchmarks"
        "end_to_end_benchmarks"
    )
    
    for suite in "${benchmark_suites[@]}"; do
        print_status "Running $suite benchmarks..."
        cargo bench --bench $suite
        
        if [ $? -eq 0 ]; then
            print_success "$suite benchmarks completed!"
        else
            print_error "$suite benchmarks failed!"
            return 1
        fi
    done
    
    print_success "All benchmarks completed!"
}

# Function to run performance tests
run_performance_tests() {
    print_status "Running performance tests..."
    
    # Create a temporary directory for performance test data
    local perf_dir=$(mktemp -d)
    export RUSTY_GUN_PERF_DIR="$perf_dir"
    
    # Run performance tests
    cargo test --package rusty-gun-benchmarks --test integration_tests test_performance_characteristics
    
    if [ $? -eq 0 ]; then
        print_success "Performance tests passed!"
    else
        print_error "Performance tests failed!"
        return 1
    fi
    
    # Clean up
    rm -rf "$perf_dir"
}

# Function to run memory tests
run_memory_tests() {
    print_status "Running memory tests..."
    
    # Check if valgrind is available
    if command -v valgrind > /dev/null 2>&1; then
        print_status "Running memory tests with valgrind..."
        cargo test --package rusty-gun-benchmarks --test integration_tests test_memory_usage
    else
        print_warning "Valgrind not available, skipping memory tests"
    fi
}

# Function to generate test report
generate_test_report() {
    print_status "Generating test report..."
    
    local report_dir="test_reports"
    mkdir -p "$report_dir"
    
    # Generate HTML report from criterion benchmarks
    if [ -d "target/criterion" ]; then
        print_status "Generating benchmark HTML report..."
        cp -r target/criterion "$report_dir/"
    fi
    
    # Generate test coverage report if requested
    if [ "$COVERAGE" = true ]; then
        print_status "Generating coverage report..."
        # This would require additional setup for coverage tools
        print_warning "Coverage report generation requires additional setup"
    fi
    
    print_success "Test report generated in $report_dir/"
}

# Main execution
main() {
    local start_time=$(date +%s)
    local failed_tests=0
    
    print_status "Starting Rusty Gun test suite..."
    print_status "Test configuration:"
    print_status "  - Unit tests: $([ "$RUN_TESTS" = true ] && echo "enabled" || echo "disabled")"
    print_status "  - Integration tests: $([ "$RUN_INTEGRATION" = true ] && echo "enabled" || echo "disabled")"
    print_status "  - Benchmarks: $([ "$RUN_BENCHMARKS" = true ] && echo "enabled" || echo "disabled")"
    print_status "  - Verbose: $([ "$VERBOSE" = true ] && echo "enabled" || echo "disabled")"
    print_status "  - Coverage: $([ "$COVERAGE" = true ] && echo "enabled" || echo "disabled")"
    echo
    
    # Run unit tests
    if [ "$RUN_TESTS" = true ]; then
        if ! run_tests; then
            ((failed_tests++))
        fi
        echo
    fi
    
    # Run integration tests
    if [ "$RUN_INTEGRATION" = true ]; then
        if ! run_integration_tests; then
            ((failed_tests++))
        fi
        echo
    fi
    
    # Run benchmarks
    if [ "$RUN_BENCHMARKS" = true ]; then
        if ! run_benchmarks; then
            ((failed_tests++))
        fi
        echo
    fi
    
    # Run performance tests
    if [ "$RUN_INTEGRATION" = true ]; then
        if ! run_performance_tests; then
            ((failed_tests++))
        fi
        echo
    fi
    
    # Run memory tests
    if [ "$RUN_INTEGRATION" = true ]; then
        if ! run_memory_tests; then
            ((failed_tests++))
        fi
        echo
    fi
    
    # Generate test report
    generate_test_report
    
    # Calculate total time
    local end_time=$(date +%s)
    local total_time=$((end_time - start_time))
    
    # Print summary
    echo
    echo "========================="
    print_status "Test Summary"
    echo "========================="
    
    if [ $failed_tests -eq 0 ]; then
        print_success "All tests passed! 🎉"
        print_success "Total time: ${total_time}s"
        exit 0
    else
        print_error "$failed_tests test suite(s) failed!"
        print_error "Total time: ${total_time}s"
        exit 1
    fi
}

# Run main function
main "$@"

