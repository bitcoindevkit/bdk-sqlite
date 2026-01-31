# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0]

### Fixed

- fix: Respect null columns when reading `tx` table

### Changed

- schema: Add migration `0002_schema.up.sql` [#14](https://github.com/bitcoindevkit/bdk-sqlite/pull/14)
 
**This release includes a migration that changes the database schema in 2 ways**:

- The `block` table's PRIMARY KEY is changed to `height`; previously it was `(height, hash)`
- The type of `anchor.block_hash` column is changed to TEXT; previously it was INTEGER

## [0.4.3]

### Fixed

- fix: Avoid inserting rows of duplicate height into `block` table [#9](https://github.com/bitcoindevkit/bdk-sqlite/pull/9)

### Changed

- feat: Make `Store::migrate` public
- deps: Bump `bdk_wallet` to 2.3.0

[unreleased]: https://github.com/bitcoindevkit/bdk-sqlite/compare/0.5.0...HEAD
[0.5.0]: https://github.com/bitcoindevkit/bdk-sqlite/releases/tag/0.5.0
[0.4.3]: https://github.com/bitcoindevkit/bdk-sqlite/releases/tag/0.4.3
