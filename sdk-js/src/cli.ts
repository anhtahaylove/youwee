#!/usr/bin/env bun

import { basename, relative, resolve } from 'node:path';
import { buildPluginPackage, packPluginPackage } from './packager';

function printHelp() {
  console.log(`youwee-sdk

Usage:
  bunx youwee-sdk build [plugin-root]
  bunx youwee-sdk pack [plugin-root]

Commands:
  build   Validate and bundle the plugin into dist/plugin.cjs
  pack    Build and package the plugin into release/<slug>-<version>.ywp
`);
}

async function main() {
  const [, , command, maybePath] = process.argv;
  if (!command || command === '--help' || command === '-h') {
    printHelp();
    return;
  }

  const cwd = maybePath ? resolve(maybePath) : process.cwd();

  if (command === 'build') {
    const result = await buildPluginPackage({ cwd });
    console.log(`Built ${result.runtimeManifest.name} -> ${relative(cwd, result.distEntrypoint)}`);
    return;
  }

  if (command === 'pack') {
    const result = await packPluginPackage({ cwd });
    console.log(`Packed ${result.manifest.name} -> ${basename(result.packagePath)}`);
    console.log(`Package path: ${result.packagePath}`);
    console.log(`Checksum: ${result.packageChecksum}`);
    return;
  }

  printHelp();
  process.exitCode = 1;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
