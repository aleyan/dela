#!/usr/bin/env node

// Sets the version in all npm package.json files to the given version.
// Usage: node scripts/npm_set_versions.js <version>

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
if (!version) {
  console.error('Usage: npm_set_versions.js <version>');
  process.exit(1);
}

const packages = [
  'npm/dela',
  'npm/dela-darwin-amd64',
  'npm/dela-darwin-arm64',
  'npm/dela-linux-amd64',
  'npm/dela-linux-arm64'
];

packages.forEach(dir => {
  const file = path.join(dir, 'package.json');
  const pkg = JSON.parse(fs.readFileSync(file, 'utf8'));
  pkg.version = version;
  if (pkg.optionalDependencies) {
    for (const dep of Object.keys(pkg.optionalDependencies)) {
      pkg.optionalDependencies[dep] = version;
    }
  }
  fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + '\n');
});
