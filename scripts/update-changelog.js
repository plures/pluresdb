#!/usr/bin/env node

/**
 * Updates CHANGELOG.md with commits since the last tag
 * This script is meant to be run during CI before version bumping
 * 
 * Usage: node scripts/update-changelog.js <version>
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

function run(cmd) {
  try {
    return execSync(cmd, { encoding: 'utf8', stdio: 'pipe' }).trim();
  } catch (e) {
    return null;
  }
}

function getCommitsSinceLastTag() {
  const lastTag = run('git describe --tags --abbrev=0 2>/dev/null');
  if (!lastTag) {
    console.log('No previous tags found, analyzing all commits');
    const commits = run('git log --format="%s"');
    return commits ? commits.split('\n').filter(Boolean) : [];
  }

  const range = `${lastTag}..HEAD`;
  const commits = run(`git log --format="%s" ${range}`);
  if (!commits) {
    console.log('No commits since last tag');
    return [];
  }

  return commits.split('\n').filter(Boolean);
}

function categorizeCommits(commits) {
  const categories = {
    feat: [],
    fix: [],
    docs: [],
    style: [],
    refactor: [],
    test: [],
    chore: [],
    perf: [],
    ci: [],
    build: [],
    other: [],
  };

  commits.forEach((commit) => {
    // Skip release commits
    if (commit.match(/^chore\(release\):/)) {
      return;
    }

    const match = commit.match(/^(\w+)(\(.+\))?: (.+)/);
    if (match) {
      const [, type, , description] = match;
      if (categories[type]) {
        categories[type].push(description);
      } else {
        categories.other.push(commit);
      }
    } else {
      categories.other.push(commit);
    }
  });

  return categories;
}

function generateChangelogEntry(version, categories) {
  const today = new Date().toISOString().split('T')[0];
  let entry = `## [${version}] - ${today}\n\n`;
  let hasContent = false;

  if (categories.feat.length > 0) {
    entry += '### Added\n\n';
    categories.feat.forEach((item) => {
      entry += `- ${item}\n`;
    });
    entry += '\n';
    hasContent = true;
  }

  if (categories.fix.length > 0) {
    entry += '### Fixed\n\n';
    categories.fix.forEach((item) => {
      entry += `- ${item}\n`;
    });
    entry += '\n';
    hasContent = true;
  }

  if (categories.refactor.length > 0 || categories.style.length > 0 || categories.perf.length > 0) {
    entry += '### Changed\n\n';
    [...categories.refactor, ...categories.style, ...categories.perf].forEach((item) => {
      entry += `- ${item}\n`;
    });
    entry += '\n';
    hasContent = true;
  }

  if (categories.docs.length > 0) {
    entry += '### Documentation\n\n';
    categories.docs.forEach((item) => {
      entry += `- ${item}\n`;
    });
    entry += '\n';
    hasContent = true;
  }

  if (categories.test.length > 0 || categories.chore.length > 0 || categories.ci.length > 0 || categories.build.length > 0) {
    entry += '### Developer Experience\n\n';
    [...categories.test, ...categories.chore, ...categories.ci, ...categories.build].forEach((item) => {
      entry += `- ${item}\n`;
    });
    entry += '\n';
    hasContent = true;
  }

  if (categories.other.length > 0) {
    entry += '### Other\n\n';
    categories.other.forEach((item) => {
      entry += `- ${item}\n`;
    });
    entry += '\n';
    hasContent = true;
  }

  if (!hasContent) {
    entry += '- Maintenance and improvements\n\n';
  }

  return entry;
}

function updateChangelog(newEntry) {
  const changelogPath = path.join(process.cwd(), 'CHANGELOG.md');
  
  if (!fs.existsSync(changelogPath)) {
    console.log('CHANGELOG.md not found, creating new one');
    const content = `# Changelog\n\nAll notable changes to this project will be documented in this file.\n\n${newEntry}`;
    fs.writeFileSync(changelogPath, content);
    console.log('Created CHANGELOG.md');
    return;
  }

  const content = fs.readFileSync(changelogPath, 'utf8');
  const lines = content.split('\n');
  let insertIndex = -1;

  // Find the position after the header and before the first version entry
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].startsWith('## [') && !lines[i].includes('[Unreleased]')) {
      insertIndex = i;
      break;
    }
  }

  if (insertIndex === -1) {
    // No previous releases found, insert after any "Unreleased" section
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].startsWith('## [Unreleased]')) {
        // Skip the unreleased section
        let j = i + 1;
        while (j < lines.length && !lines[j].startsWith('##')) {
          j++;
        }
        insertIndex = j;
        break;
      } else if (lines[i].startsWith('# ')) {
        // Found the main header, insert after it
        insertIndex = i + 2;
        break;
      }
    }
  }

  if (insertIndex === -1) {
    // Last resort: append at the end
    lines.push('', newEntry);
  } else {
    // Insert the new entry
    lines.splice(insertIndex, 0, newEntry);
  }

  fs.writeFileSync(changelogPath, lines.join('\n'));
  console.log('✅ Updated CHANGELOG.md');
}

function main() {
  const version = process.argv[2];
  if (!version) {
    console.error('Usage: node scripts/update-changelog.js <version>');
    process.exit(1);
  }

  const commits = getCommitsSinceLastTag();
  if (commits.length === 0) {
    console.log('No commits to add to changelog, creating minimal entry');
  }

  const categories = categorizeCommits(commits);
  const entry = generateChangelogEntry(version, categories);
  updateChangelog(entry);
}

main();
