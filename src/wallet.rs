//! [`AsyncWalletPersister`] implementation for the async [`Store`].

use std::{collections::BTreeMap, pin::Pin, str::FromStr};

use bdk_chain::bitcoin;
use bdk_chain::miniscript;
use bdk_wallet::{AsyncWalletPersister, ChangeSet, KeychainKind};
use bitcoin::Network;
use miniscript::descriptor::{Descriptor, DescriptorPublicKey};
use sqlx::Row;

use crate::Store;

impl Store {
    /// Write changeset.
    pub async fn write_changeset(&self, changeset: &ChangeSet) {
        if let Some(network) = changeset.network {
            self.write_network(network).await;
        }

        let mut descriptors = BTreeMap::new();
        if let Some(ref descriptor) = changeset.descriptor {
            descriptors.insert(KeychainKind::External, descriptor.clone());
        }
        if let Some(ref change_descriptor) = changeset.change_descriptor {
            descriptors.insert(KeychainKind::Internal, change_descriptor.clone());
        }
        self.write_keychain_descriptors(descriptors).await;

        self.write_local_chain(&changeset.local_chain).await;
        self.write_tx_graph(&changeset.tx_graph).await;
        self.write_keychain_txout(&changeset.indexer).await;
    }

    /// Write network.
    pub async fn write_network(&self, network: Network) {
        let mut conn = self.pool.acquire().await.unwrap();

        sqlx::query("insert into network(network) values($1)")
            .bind(network.to_string())
            .execute(&mut *conn)
            .await
            .unwrap();
    }

    /// Write keychain descriptors.
    pub async fn write_keychain_descriptors(
        &self,
        descriptors: BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>>,
    ) {
        let mut conn = self.pool.acquire().await.unwrap();

        for (keychain, descriptor) in descriptors {
            let keychain = match keychain {
                KeychainKind::External => 0u8,
                KeychainKind::Internal => 1,
            };
            sqlx::query("insert into keychain(keychain, descriptor) values($1, $2)")
                .bind(keychain)
                .bind(descriptor.to_string())
                .execute(&mut *conn)
                .await
                .unwrap();
        }
    }

    /// Read changeset.
    pub async fn read_changeset(&self) -> ChangeSet {
        let mut changeset = ChangeSet::default();

        changeset.network = self.read_network().await;

        let descriptors = self.read_keychain_descriptors().await;
        changeset.descriptor = descriptors.get(&KeychainKind::External).cloned();
        changeset.change_descriptor = descriptors.get(&KeychainKind::Internal).cloned();

        changeset.tx_graph = self.read_tx_graph().await;
        changeset.local_chain = self.read_local_chain().await;
        changeset.indexer = self.read_keychain_txout().await;

        changeset
    }

    /// Read network.
    pub async fn read_network(&self) -> Option<Network> {
        let mut conn = self.pool.acquire().await.unwrap();

        let row = sqlx::query("select network from network")
            .fetch_optional(&mut *conn)
            .await
            .unwrap();

        row.and_then(|row| {
            let network: String = row.get("network");
            Some(network.parse().unwrap())
        })
    }

    /// Read keychain descriptors.
    pub async fn read_keychain_descriptors(
        &self,
    ) -> BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>> {
        let mut conn = self.pool.acquire().await.unwrap();

        let mut descriptors = BTreeMap::new();

        let rows = sqlx::query("select keychain, descriptor from keychain")
            .fetch_all(&mut *conn)
            .await
            .unwrap();
        for row in rows {
            let keychain: u8 = row.get("keychain");
            let keychain = match keychain {
                0 => KeychainKind::External,
                1 => KeychainKind::Internal,
                _ => panic!("unsupported keychain kind"),
            };
            let descriptor: String = row.get("descriptor");
            let descriptor = Descriptor::from_str(&descriptor).unwrap();
            descriptors.insert(keychain, descriptor);
        }

        descriptors
    }
}

type FutureResult<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + 'a + Send>>;

impl AsyncWalletPersister for Store {
    type Error = ();

    fn initialize<'a>(persister: &'a mut Self) -> FutureResult<'a, ChangeSet, Self::Error>
    where
        Self: 'a,
    {
        Box::pin(async {
            persister.migrate().await;
            Ok(persister.read_changeset().await)
        })
    }

    fn persist<'a>(
        persister: &'a mut Self,
        changeset: &'a ChangeSet,
    ) -> FutureResult<'a, (), Self::Error>
    where
        Self: 'a,
    {
        Box::pin(async { Ok(persister.write_changeset(changeset).await) })
    }
}
