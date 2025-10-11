# `bdk_sqlite`

This crate features the [`Store`] type which provides async read and write methods of persisting BDK change sets by way of [`sqlx`].

## Example

```rust,no_run
use bdk_sqlite::Store;
use bdk_wallet::bitcoin;
use bdk_wallet::{KeychainKind, Wallet};
use bitcoin::Network;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create `Store`.
    let mut db = Store::new("test.db").await?;

    let descriptor = "wpkh([e273fe42/84'/1'/0']tpubDCmr3Luq75npLaYmRqqW1rLfSbfpnBXwLwAmUbR333fp95wjCHar3zoc9zSWovZFwrWr53mm3NTVqt6d1Pt6G26uf4etQjc3Pr5Hxe9QEQ2/0/*)";
    let change_descriptor = "wpkh([e273fe42/84'/1'/0']tpubDCmr3Luq75npLaYmRqqW1rLfSbfpnBXwLwAmUbR333fp95wjCHar3zoc9zSWovZFwrWr53mm3NTVqt6d1Pt6G26uf4etQjc3Pr5Hxe9QEQ2/1/*)";

    // Create `Wallet`.
    let mut wallet = Wallet::create(descriptor, change_descriptor)
        .network(Network::Signet)
        .create_wallet_async(&mut db)
        .await?;

    println!(
        "Address: {}",
        wallet.reveal_next_address(KeychainKind::External),
    );

    // Persist wallet state to SQLite database.
    wallet.persist_async(&mut db).await?;

    Ok(())
}
```

## Features

* `wallet` - Provides access to the [`AsyncWalletPersister`] implementation for [`Store`] (enabled by default).

## MSRV

The Minimum Supported Rust Version (MSRV) is 1.85.0.

[`sqlx`]: https://docs.rs/sqlx/latest/sqlx/
[`AsyncWalletPersister`]: https://docs.rs/bdk_wallet/latest/bdk_wallet/trait.AsyncWalletPersister.html
