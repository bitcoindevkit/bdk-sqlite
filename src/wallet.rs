//! [`AsyncWalletPersister`] implementation for the async [`Store`].

use std::{collections::BTreeMap, pin::Pin, str::FromStr};

use bdk_chain::bitcoin;
use bdk_chain::miniscript;
use bdk_wallet::{AsyncWalletPersister, ChangeSet, KeychainKind, locked_outpoints};
use bitcoin::Network;
use bitcoin::OutPoint;
use miniscript::descriptor::{Descriptor, DescriptorPublicKey};
use sqlx::{Row, sqlite::SqliteConnection};

use crate::Error;
use crate::Store;

impl Store {
    /// Write changeset.
    pub async fn write_changeset(&self, changeset: &ChangeSet) -> Result<(), Error> {
        let mut txn = self.pool.begin().await?;

        if let Some(network) = changeset.network {
            Self::write_network(&mut txn, network).await?;
        }

        let mut descriptors = BTreeMap::new();
        if let Some(ref descriptor) = changeset.descriptor {
            descriptors.insert(KeychainKind::External, descriptor.clone());
        }
        if let Some(ref change_descriptor) = changeset.change_descriptor {
            descriptors.insert(KeychainKind::Internal, change_descriptor.clone());
        }
        Self::write_keychain_descriptors(&mut txn, descriptors).await?;

        Self::write_local_chain(&mut txn, &changeset.local_chain).await?;
        Self::write_tx_graph(&mut txn, &changeset.tx_graph).await?;
        Self::write_keychain_txout(&mut txn, &changeset.indexer).await?;
        Self::write_locked_outpoints(&mut txn, &changeset.locked_outpoints).await?;

        txn.commit().await?;
        Ok(())
    }

    /// Write network.
    pub async fn write_network(conn: &mut SqliteConnection, network: Network) -> Result<(), Error> {
        sqlx::query("INSERT OR IGNORE INTO network(network) VALUES($1)")
            .bind(network.to_string())
            .execute(&mut *conn)
            .await?;

        Ok(())
    }

    /// Write keychain descriptors.
    pub async fn write_keychain_descriptors(
        conn: &mut SqliteConnection,
        descriptors: BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>>,
    ) -> Result<(), Error> {
        for (keychain, descriptor) in descriptors {
            let keychain = match keychain {
                KeychainKind::External => 0u8,
                KeychainKind::Internal => 1,
            };
            sqlx::query("INSERT OR IGNORE INTO keychain(keychain, descriptor) VALUES($1, $2)")
                .bind(keychain)
                .bind(descriptor.to_string())
                .execute(&mut *conn)
                .await?;
        }

        Ok(())
    }

    /// Read changeset.
    pub async fn read_changeset(&self) -> Result<ChangeSet, Error> {
        let mut txn = self.pool.begin().await?;

        let network = Self::read_network(&mut txn).await?;

        let descriptors = Self::read_keychain_descriptors(&mut txn).await?;
        let descriptor = descriptors.get(&KeychainKind::External).cloned();
        let change_descriptor = descriptors.get(&KeychainKind::Internal).cloned();

        let tx_graph = Self::read_tx_graph(&mut txn).await?;
        let local_chain = Self::read_local_chain(&mut txn).await?;
        let indexer = Self::read_keychain_txout(&mut txn).await?;
        let locked_outpoints = Self::read_locked_outpoints(&mut txn).await?;

        txn.commit().await?;
        Ok(ChangeSet {
            network,
            descriptor,
            change_descriptor,
            tx_graph,
            local_chain,
            indexer,
            locked_outpoints,
        })
    }

    /// Read network.
    pub async fn read_network(conn: &mut SqliteConnection) -> Result<Option<Network>, Error> {
        let row = sqlx::query("SELECT network FROM network")
            .fetch_optional(&mut *conn)
            .await?;

        row.map(|row| {
            let s: String = row.get("network");
            s.parse().map_err(Error::ParseNetwork)
        })
        .transpose()
    }

    /// Read keychain descriptors.
    pub async fn read_keychain_descriptors(
        conn: &mut SqliteConnection,
    ) -> Result<BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>>, Error> {
        let mut descriptors = BTreeMap::new();

        let rows = sqlx::query("SELECT keychain, descriptor FROM keychain")
            .fetch_all(&mut *conn)
            .await?;
        for row in rows {
            let keychain: u8 = row.get("keychain");
            let keychain = match keychain {
                0 => KeychainKind::External,
                1 => KeychainKind::Internal,
                _ => {
                    debug_assert!(false, "keychain must map to a value of 0 or 1");
                    continue;
                }
            };
            let descriptor: String = row.get("descriptor");
            let descriptor = Descriptor::from_str(&descriptor)?;
            descriptors.insert(keychain, descriptor);
        }

        Ok(descriptors)
    }

    /// Write locked outpoints.
    pub async fn write_locked_outpoints(
        conn: &mut SqliteConnection,
        locked_outpoints: &locked_outpoints::ChangeSet,
    ) -> Result<(), Error> {
        for (&outpoint, &is_locked) in &locked_outpoints.outpoints {
            let OutPoint { txid, vout } = outpoint;
            if is_locked {
                sqlx::query("INSERT OR IGNORE INTO locked_outpoint(txid, vout) VALUES($1, $2)")
                    .bind(txid.to_string())
                    .bind(vout)
                    .execute(&mut *conn)
                    .await?;
            } else {
                sqlx::query("DELETE FROM locked_outpoint WHERE txid = $1 AND vout = $2")
                    .bind(txid.to_string())
                    .bind(vout)
                    .execute(&mut *conn)
                    .await?;
            }
        }

        Ok(())
    }

    /// Read locked outpoints.
    pub async fn read_locked_outpoints(
        conn: &mut SqliteConnection,
    ) -> Result<locked_outpoints::ChangeSet, Error> {
        let mut changeset = locked_outpoints::ChangeSet::default();

        let rows = sqlx::query("SELECT txid, vout FROM locked_outpoint")
            .fetch_all(&mut *conn)
            .await?;
        for row in rows {
            let txid: String = row.get("txid");
            let txid: bitcoin::Txid = txid.parse()?;
            let vout: u32 = row.get("vout");
            let outpoint = OutPoint { txid, vout };
            changeset.outpoints.insert(outpoint, true);
        }

        Ok(changeset)
    }
}

type FutureResult<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + 'a + Send>>;

impl AsyncWalletPersister for Store {
    type Error = crate::Error;

    fn initialize<'a>(persister: &'a mut Self) -> FutureResult<'a, ChangeSet, Self::Error>
    where
        Self: 'a,
    {
        Box::pin(async {
            persister.migrate().await?;
            persister.read_changeset().await
        })
    }

    fn persist<'a>(
        persister: &'a mut Self,
        changeset: &'a ChangeSet,
    ) -> FutureResult<'a, (), Self::Error>
    where
        Self: 'a,
    {
        Box::pin(async { persister.write_changeset(changeset).await })
    }
}
