# Changelog

All notable changes to `youwee-sdk` should be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.0.0] - 2026-05-18

### Added
- **Stable 1.0 release** - Finalized the workspace-to-`.ywp` plugin workflow, live workspace debugging model, packaged plugin runtime, CLI build/pack commands, and publish-ready SDK docs for npm distribution.

## [0.1.0] - 2026-05-16

### Added
- **Runtime bootstrap** - Added `runtime-cli` so plugin packages do not need per-plugin runner files.
- **Hook contract** - Added typed trigger contracts for download and processing lifecycle hooks.
- **Capability bridge** - Added accessors for Youwee runtime metadata, tool paths, AI configuration, filesystem helpers, and HTTP helpers.
- **Manifest helpers** - Added manifest validation and package template helpers for plugin authoring workflows.
- **Compatibility policy** - Added app-version and SDK-version compatibility helpers and enforcement-ready manifest fields.
