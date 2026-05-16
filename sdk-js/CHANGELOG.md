# Changelog

All notable changes to `youwee-sdk` should be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- **Initial SDK package** - Added a TypeScript-authored JavaScript plugin SDK with shared runtime bootstrap, typed contexts, manifest helpers, AI bridge, filesystem bridge, HTTP bridge, compatibility helpers, and schema validation helpers.

## [0.1.0] - 2026-05-16

### Added
- **Runtime bootstrap** - Added `runtime-cli` so plugin packages do not need per-plugin runner files.
- **Hook contract** - Added typed trigger contracts for download and processing lifecycle hooks.
- **Capability bridge** - Added accessors for Youwee runtime metadata, tool paths, AI configuration, filesystem helpers, and HTTP helpers.
- **Manifest helpers** - Added manifest validation and package template helpers for plugin authoring workflows.
- **Compatibility policy** - Added app-version and SDK-version compatibility helpers and enforcement-ready manifest fields.
