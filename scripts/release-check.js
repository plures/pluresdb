#!/usr/bin/env node

/**
 * Pre-release validation checks
 * Ensures the repository is ready for a release
 * 
 * Usage: node scripts/release-check.js
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

function run(cmd, ignoreError = false) {
  try {
    return execSync(cmd, { encoding: 'utf8', stdio: 'pipe' }).trim();
  } catch (e) {
    if (ignoreError) return null;
    throw e;
  }
}

function checkGitStatus() {
  console.log('🔍 Checking git status...');
  const status = run('git status --porcelain', true);
  
  if (status && status.length > 0) {
    console.warn('⚠️  Warning: Working directory has uncommitted changes');
    console.log(status);
  } else {
    console.log('✅ Working directory is clean');
  }
}

function checkPackageVersions() {
  console.log('\n🔍 Checking package versions...');
  
  const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  const cargoToml = fs.readFileSync('Cargo.toml', 'utf8');
  
  const packageVersion = packageJson.version;
  const cargoMatch = cargoToml.match(/^version\s*=\s*"([^"]+)"/m);
  const cargoVersion = cargoMatch ? cargoMatch[1] : null;
  
  console.log(`  package.json: ${packageVersion}`);
  console.log(`  Cargo.toml:   ${cargoVersion || 'not found'}`);
  
  if (cargoVersion && packageVersion !== cargoVersion) {
    console.error('❌ Version mismatch between package.json and Cargo.toml');
    return false;
  }
  
  console.log('✅ Package versions are consistent');
  return true;
}

function checkChangelog() {
  console.log('\n🔍 Checking CHANGELOG.md...');
  
  if (!fs.existsSync('CHANGELOG.md')) {
    console.error('❌ CHANGELOG.md not found');
    return false;
  }
  
  const changelog = fs.readFileSync('CHANGELOG.md', 'utf8');
  const packageJson = JSON.parse(fs.readFileSync('package.json', 'utf8'));
  const currentVersion = packageJson.version;
  
  if (!changelog.includes(`## [${currentVersion}]`)) {
    console.warn(`⚠️  Warning: Current version ${currentVersion} not found in CHANGELOG.md`);
  } else {
    console.log(`✅ CHANGELOG.md includes current version ${currentVersion}`);
  }
  
  return true;
}

function checkCommitMessages() {
  console.log('\n🔍 Checking recent commit messages...');
  
  const lastTag = run('git describe --tags --abbrev=0 2>/dev/null', true);
  const range = lastTag ? `${lastTag}..HEAD` : '--all';
  
  const commits = run(`git log --format="%s" ${range}`, true);
  
  if (!commits) {
    console.log('ℹ️  No commits to check');
    return true;
  }
  
  const commitLines = commits.split('\n').filter(Boolean);
  const conventionalTypes = ['feat', 'fix', 'docs', 'style', 'refactor', 'perf', 'test', 'chore', 'ci', 'build', 'revert'];
  
  let conventionalCount = 0;
  commitLines.forEach(commit => {
    const match = commit.match(/^(\w+)(\(.+\))?:/);
    if (match && conventionalTypes.includes(match[1])) {
      conventionalCount++;
    }
  });
  
  const percentage = (conventionalCount / commitLines.length * 100).toFixed(0);
  console.log(`  ${conventionalCount}/${commitLines.length} commits follow conventional format (${percentage}%)`);
  
  if (conventionalCount === 0) {
    console.warn('⚠️  Warning: No conventional commits found');
  } else {
    console.log('✅ Found conventional commits');
  }
  
  return true;
}

function checkBuildStatus() {
  console.log('\n🔍 Checking if project builds...');
  
  try {
    // Check if npm build works
    console.log('  Running npm run build:lib...');
    run('npm run build:lib');
    console.log('✅ Build successful');
    return true;
  } catch (e) {
    console.error('❌ Build failed');
    console.error(e.message);
    return false;
  }
}

function checkTests() {
  console.log('\n🔍 Checking tests...');
  
  try {
    console.log('  Running tests...');
    run('npm test');
    console.log('✅ Tests passed');
    return true;
  } catch (e) {
    console.error('❌ Tests failed');
    console.error(e.message);
    return false;
  }
}

function main() {
  console.log('🚀 Running pre-release checks for pluresdb\n');
  
  let allPassed = true;
  
  try {
    checkGitStatus();
    
    if (!checkPackageVersions()) {
      allPassed = false;
    }
    
    if (!checkChangelog()) {
      allPassed = false;
    }
    
    checkCommitMessages();
    
    // Skip build and test checks in CI for now
    if (process.env.CI !== 'true') {
      if (!checkBuildStatus()) {
        allPassed = false;
      }
      
      if (!checkTests()) {
        allPassed = false;
      }
    } else {
      console.log('\nℹ️  Skipping build and test checks in CI');
    }
    
    console.log('\n' + '='.repeat(50));
    if (allPassed) {
      console.log('✅ All pre-release checks passed!');
      process.exit(0);
    } else {
      console.log('❌ Some pre-release checks failed');
      process.exit(1);
    }
  } catch (e) {
    console.error('\n❌ Pre-release check error:', e.message);
    process.exit(1);
  }
}

main();
