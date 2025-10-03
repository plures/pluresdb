#!/usr/bin/env -S deno run -A --unstable-kv

// @ts-nocheck

interface TestSuite {
  name: string;
  command: string;
  description: string;
  timeout?: number;
}

interface TestResults {
  suite: string;
  passed: boolean;
  duration: number;
  output: string;
  error?: string;
}

class TestRunner {
  private results: TestResults[] = [];

  async runTestSuite(suite: TestSuite): Promise<TestResults> {
    console.log(`\n🧪 Running ${suite.name}...`);
    console.log(`   ${suite.description}`);

    const startTime = performance.now();

    try {
      const command = new Deno.Command("deno", {
        args: suite.command.split(" "),
        stdout: "piped",
        stderr: "piped",
      });

      const { code, stdout, stderr } = await command.output();
      const duration = performance.now() - startTime;

      const output = new TextDecoder().decode(stdout);
      const error = new TextDecoder().decode(stderr);

      const result: TestResults = {
        suite: suite.name,
        passed: code === 0,
        duration,
        output,
        error: code !== 0 ? error : undefined,
      };

      this.results.push(result);

      if (result.passed) {
        console.log(`   ✅ Passed (${duration.toFixed(2)}ms)`);
      } else {
        console.log(`   ❌ Failed (${duration.toFixed(2)}ms)`);
        console.log(`   Error: ${result.error}`);
      }

      return result;
    } catch (error) {
      const duration = performance.now() - startTime;

      const result: TestResults = {
        suite: suite.name,
        passed: false,
        duration,
        output: "",
        error: error.message,
      };

      this.results.push(result);

      console.log(`   ❌ Failed (${duration.toFixed(2)}ms)`);
      console.log(`   Error: ${error.message}`);

      return result;
    }
  }

  printSummary() {
    console.log("\n" + "=".repeat(80));
    console.log("TEST SUMMARY");
    console.log("=".repeat(80));

    const passed = this.results.filter((r) => r.passed).length;
    const total = this.results.length;
    const totalDuration = this.results.reduce((sum, r) => sum + r.duration, 0);

    console.log(`Total Tests: ${total}`);
    console.log(`Passed: ${passed}`);
    console.log(`Failed: ${total - passed}`);
    console.log(`Total Duration: ${totalDuration.toFixed(2)}ms`);
    console.log(`Success Rate: ${((passed / total) * 100).toFixed(1)}%`);

    console.log("\nDetailed Results:");
    this.results.forEach((result) => {
      const status = result.passed ? "✅" : "❌";
      console.log(`  ${status} ${result.suite} (${result.duration.toFixed(2)}ms)`);
    });

    if (passed < total) {
      console.log("\nFailed Tests:");
      this.results
        .filter((r) => !r.passed)
        .forEach((result) => {
          console.log(`\n❌ ${result.suite}:`);
          console.log(`   Error: ${result.error}`);
        });
    }
  }
}

async function main() {
  console.log("🚀 PluresDB Test Suite Runner");
  console.log("===============================");

  const testSuites: TestSuite[] = [
    {
      name: "Unit Tests",
      command: "test -A --unstable-kv --parallel src/tests/unit/",
      description: "Core functionality tests (CRUD, subscriptions, vector search)",
      timeout: 30000,
    },
    {
      name: "Integration Tests",
      command: "test -A --unstable-kv --parallel src/tests/integration/",
      description: "Mesh networking and API server tests",
      timeout: 60000,
    },
    {
      name: "Performance Tests",
      command: "test -A --unstable-kv --parallel src/tests/performance/",
      description: "Load testing and performance validation",
      timeout: 120000,
    },
    {
      name: "Security Tests",
      command: "test -A --unstable-kv --parallel src/tests/security/",
      description: "Input validation and security testing",
      timeout: 30000,
    },
    {
      name: "Code Quality",
      command: "lint src/",
      description: "Code linting and style checks",
      timeout: 10000,
    },
    {
      name: "Type Checking",
      command: "check src/main.ts",
      description: "TypeScript type checking",
      timeout: 15000,
    },
  ];

  const runner = new TestRunner();

  for (const suite of testSuites) {
    await runner.runTestSuite(suite);
  }

  runner.printSummary();

  const failedTests = runner.results.filter((r) => !r.passed).length;
  if (failedTests > 0) {
    console.log(`\n❌ ${failedTests} test suite(s) failed!`);
    Deno.exit(1);
  } else {
    console.log("\n🎉 All tests passed!");
  }
}

if (import.meta.main) {
  await main();
}
