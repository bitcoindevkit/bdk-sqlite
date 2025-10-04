//! [`AsyncWalletPersister`] implementation for the async [`Store`].

use std::{collections::BTreeMap, pin::Pin, str::FromStr};

use bdk_chain::bitcoin;
use bdk_chain::miniscript;
use bdk_wallet::{AsyncWalletPersister, ChangeSet, KeychainKind};
use bitcoin::Network;
use miniscript::descriptor::{Descriptor, DescriptorPublicKey};
use sqlx::Row;

use crate::Error;
use crate::Store;

impl Store {
    /// Write changeset.
    pub async fn write_changeset(&self, changeset: &ChangeSet) -> Result<(), Error> {
        if let Some(network) = changeset.network {
            self.write_network(network).await?;
        }

        let mut descriptors = BTreeMap::new();
        if let Some(ref descriptor) = changeset.descriptor {
            descriptors.insert(KeychainKind::External, descriptor.clone());
        }
        if let Some(ref change_descriptor) = changeset.change_descriptor {
            descriptors.insert(KeychainKind::Internal, change_descriptor.clone());
        }
        self.write_keychain_descriptors(descriptors).await?;

        self.write_local_chain(&changeset.local_chain).await?;
        self.write_tx_graph(&changeset.tx_graph).await?;
        self.write_keychain_txout(&changeset.indexer).await?;

        Ok(())
    }

    /// Write network.
    pub async fn write_network(&self, network: Network) -> Result<(), Error> {
        sqlx::query("insert into network(network) values($1)")
            .bind(network.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Write keychain descriptors.
    pub async fn write_keychain_descriptors(
        &self,
        descriptors: BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>>,
    ) -> Result<(), Error> {
        for (keychain, descriptor) in descriptors {
            let keychain = match keychain {
                KeychainKind::External => 0u8,
                KeychainKind::Internal => 1,
            };
            sqlx::query("insert into keychain(keychain, descriptor) values($1, $2)")
                .bind(keychain)
                .bind(descriptor.to_string())
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    /// Read changeset.
    pub async fn read_changeset(&self) -> Result<ChangeSet, Error> {
        let network = self.read_network().await?;

        let descriptors = self.read_keychain_descriptors().await?;
        let descriptor = descriptors.get(&KeychainKind::External).cloned();
        let change_descriptor = descriptors.get(&KeychainKind::Internal).cloned();

        let tx_graph = self.read_tx_graph().await?;
        let local_chain = self.read_local_chain().await?;
        let indexer = self.read_keychain_txout().await?;

        Ok(ChangeSet {
            network,
            descriptor,
            change_descriptor,
            tx_graph,
            local_chain,
            indexer,
        })
    }

    /// Read network.
    pub async fn read_network(&self) -> Result<Option<Network>, Error> {
        let row = sqlx::query("select network from network")
            .fetch_optional(&self.pool)
            .await?;

        row.map(|row| {
            let s: String = row.get("network");
            s.parse().map_err(Error::ParseNetwork)
        })
        .transpose()
    }

    /// Read keychain descriptors.
    pub async fn read_keychain_descriptors(
        &self,
    ) -> Result<BTreeMap<KeychainKind, Descriptor<DescriptorPublicKey>>, Error> {
        let mut descriptors = BTreeMap::new();

        let rows = sqlx::query("select keychain, descriptor from keychain")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let keychain: u8 = row.get("keychain");
            let keychain = match keychain {
                0 => KeychainKind::External,
                1 => KeychainKind::Internal,
                _ => panic!("unsupported keychain kind"),
            };
            let descriptor: String = row.get("descriptor");
            let descriptor = Descriptor::from_str(&descriptor)?;
            descriptors.insert(keychain, descriptor);
        }

        Ok(descriptors)
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
