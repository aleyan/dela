#!/usr/bin/env node

// Sets Cargo.toml, Cargo.lock, CHANGELOG.md, and npm package.json versions.
// Usage: node scripts/set_versions.js <version>

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
if (!version) {
  console.error('Usage: set_versions.js <version>');
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`Invalid version "${version}". Expected X.Y.Z.`);
  process.exit(1);
}

const releaseDate = process.env.RELEASE_DATE || new Date().toISOString().slice(0, 10);
if (!/^\d{4}-\d{2}-\d{2}$/.test(releaseDate)) {
  console.error(`Invalid RELEASE_DATE "${releaseDate}". Expected YYYY-MM-DD.`);
  process.exit(1);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function setCargoVersion() {
  const cargoToml = 'Cargo.toml';
  const lines = fs.readFileSync(cargoToml, 'utf8').split('\n');
  let inPackageSection = false;
  let updated = false;

  const nextLines = lines.map(line => {
    if (line === '[package]') {
      inPackageSection = true;
      return line;
    }

    if (inPackageSection && line.startsWith('[')) {
      inPackageSection = false;
    }

    if (inPackageSection && line.startsWith('version = ')) {
      updated = true;
      return `version = "${version}"`;
    }

    return line;
  });

  if (!updated) {
    console.error('Could not find package version in Cargo.toml.');
    process.exit(1);
  }

  fs.writeFileSync(cargoToml, nextLines.join('\n'));
}

function setCargoLockVersion() {
  const cargoLock = 'Cargo.lock';
  const lines = fs.readFileSync(cargoLock, 'utf8').split('\n');
  let inDelaPackage = false;
  let updated = false;

  const nextLines = lines.map(line => {
    if (line === '[[package]]') {
      inDelaPackage = false;
      return line;
    }

    if (line === 'name = "dela"') {
      inDelaPackage = true;
      return line;
    }

    if (inDelaPackage && line.startsWith('version = ')) {
      updated = true;
      return `version = "${version}"`;
    }

    return line;
  });

  if (!updated) {
    console.error('Could not find dela package version in Cargo.lock.');
    process.exit(1);
  }

  fs.writeFileSync(cargoLock, nextLines.join('\n'));
}

function setChangelogVersion() {
  const changelog = 'CHANGELOG.md';
  const content = fs.readFileSync(changelog, 'utf8');
  const headingPattern = new RegExp(`^## \\[${escapeRegExp(version)}\\] - (.*)$`, 'm');
  const existingHeading = content.match(headingPattern);
  if (existingHeading) {
    if (existingHeading[1] === 'Unreleased') {
      const nextContent = content.replace(headingPattern, `## [${version}] - ${releaseDate}`);
      fs.writeFileSync(changelog, nextContent);
    }
    return;
  }

  const newEntry = `## [${version}] - ${releaseDate}\n\n`;
  const firstReleaseHeading = content.match(/^## \[/m);
  if (!firstReleaseHeading || firstReleaseHeading.index === undefined) {
    const separator = content.endsWith('\n') ? '\n' : '\n\n';
    fs.writeFileSync(changelog, `${content}${separator}${newEntry}`);
    return;
  }

  const insertAt = firstReleaseHeading.index;
  const nextContent = `${content.slice(0, insertAt)}${newEntry}${content.slice(insertAt)}`;
  fs.writeFileSync(changelog, nextContent);
}

const packages = [
  'npm/dela',
  'npm/dela-darwin-amd64',
  'npm/dela-darwin-arm64',
  'npm/dela-linux-amd64',
  'npm/dela-linux-arm64'
];

setCargoVersion();
setCargoLockVersion();
setChangelogVersion();

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
