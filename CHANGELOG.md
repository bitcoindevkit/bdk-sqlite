# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0]

### Added

- `0003_schema.up.sql` adds a `locked_outpoint` table for persisting UTXO lock state [#16](https://github.com/bitcoindevkit/bdk-sqlite/pull/16)
- Associated functions `write_locked_outpoints` and `read_locked_outpoints` are added for `Store` [#16](https://github.com/bitcoindevkit/bdk-sqlite/pull/16)
- `0004_schema.up.sql` enforces a single row constraint on `network` table [#19](https://github.com/bitcoindevkit/bdk-sqlite/pull/19)

### Changed

- Execute queries inside a `sqlx` Transaction.
  `Store` methods that previously took `&self` now accept `conn: &mut SqliteConnection`.
  Callers who invoked these directly must now supply a connection or transaction handle. [#18](https://github.com/bitcoindevkit/bdk-sqlite/pull/18)

    - `Store::write_tx_graph`
    - `Store::write_local_chain`
    - `Store::write_keychain_txout`
    - `Store::read_tx_graph`
    - `Store::read_local_chain`
    - `Store::read_keychain_txout`
    - `Store::write_network`
    - `Store::write_keychain_descriptors`
    - `Store::write_locked_outpoints`
    - `Store::read_network`
    - `Store::read_keychain_descriptors`
    - `Store::read_locked_outpoints`

- wallet: `write_keychain_descriptors` accepts as argument `&BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>>`;
  previously it took an owned value. [#18](https://github.com/bitcoindevkit/bdk-sqlite/pull/18)
- Bump `bdk_chain` to 0.23.3
- Bump `bdk_wallet` to 3.0.0
- (dev) Bump `bdk_esplora` 0.22.2

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

[unreleased]: https://github.com/bitcoindevkit/bdk-sqlite/compare/0.6.0...HEAD
[0.6.0]: https://github.com/bitcoindevkit/bdk-sqlite/releases/tag/0.6.0
[0.5.0]: https://github.com/bitcoindevkit/bdk-sqlite/releases/tag/0.5.0
[0.4.3]: https://github.com/bitcoindevkit/bdk-sqlite/releases/tag/0.4.3
