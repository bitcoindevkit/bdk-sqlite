//! [`Store`] provides async read and write methods of persisting BDK change sets by way of [`sqlx`].

use std::str::FromStr;
use std::sync::Arc;

use bdk_chain::{
    BlockId, ConfirmationBlockTime, DescriptorId, bitcoin, keychain_txout, local_chain, tx_graph,
};
use bitcoin::{Amount, BlockHash, OutPoint, ScriptBuf, Transaction, TxOut, Txid, consensus};
use sqlx::{
    Row,
    sqlite::{SqliteConnectOptions, SqlitePool as Pool},
};

use crate::Error;

/// Store.
#[derive(Debug, Clone)]
pub struct Store {
    /// Pool.
    pub(crate) pool: Pool,
}

impl Store {
    /// New in memory.
    pub async fn new_memory() -> Result<Self, Error> {
        let mut options = sqlx::sqlite::SqlitePoolOptions::new();
        // Don't test the health of the connection before returning it.
        // See docs for `Pool::acquire`.
        options = options.test_before_acquire(false);
        let pool = options.connect("sqlite::memory:").await?;

        Ok(Self { pool })
    }

    /// Create a new [`Store`] instance.
    ///
    /// This will create a new database at the given path if it doesn't exist.
    ///
    /// Note that `path` can be a filename, e.g. `foo.db` or a standard URL,
    /// e.g. `sqlite://foo.db`.
    pub async fn new(path: &str) -> Result<Self, Error> {
        let options = SqliteConnectOptions::from_str(path)?.create_if_missing(true);
        let pool = Pool::connect_with(options).await?;

        Ok(Self { pool })
    }

    /// Create a new [`Store`] from an existing [`Pool`].
    pub async fn new_pool(pool: Pool) -> Result<Self, Error> {
        let store = Self { pool };

        Ok(store)
    }

    /// Runs pending migrations against the database.
    pub async fn migrate(&self) -> Result<(), Error> {
        Ok(sqlx::migrate!().run(&self.pool).await?)
    }
}

impl Store {
    /// Write tx_graph.
    pub async fn write_tx_graph(
        &self,
        tx_graph: &tx_graph::ChangeSet<ConfirmationBlockTime>,
    ) -> Result<(), Error> {
        let txs = &tx_graph.txs;
        let txouts = &tx_graph.txouts;
        let anchors = &tx_graph.anchors;
        let first_seen = &tx_graph.first_seen;
        let last_seen = &tx_graph.last_seen;
        let last_evicted = &tx_graph.last_evicted;

        for tx in txs {
            let txid = tx.compute_txid();
            sqlx::query(
                "INSERT INTO tx(txid, tx) VALUES($1, $2) ON CONFLICT DO UPDATE SET tx = $2",
            )
            .bind(txid.to_string())
            .bind(consensus::encode::serialize(tx))
            .execute(&self.pool)
            .await?;
        }
        for (txid, t) in first_seen {
            sqlx::query("INSERT INTO tx(txid, first_seen) VALUES($1, $2) ON CONFLICT DO UPDATE SET first_seen = $2")
                .bind(txid.to_string())
                .bind(i64::try_from(*t)?)
                .execute(&self.pool)
                .await?;
        }
        for (txid, t) in last_seen {
            sqlx::query("INSERT INTO tx(txid, last_seen) VALUES($1, $2) ON CONFLICT DO UPDATE SET last_seen = $2")
                .bind(txid.to_string())
                .bind(i64::try_from(*t)?)
                .execute(&self.pool)
                .await?;
        }
        for (txid, t) in last_evicted {
            sqlx::query("INSERT INTO tx(txid, last_evicted) VALUES($1, $2) ON CONFLICT DO UPDATE SET last_evicted = $2")
                .bind(txid.to_string())
                .bind(i64::try_from(*t)?)
                .execute(&self.pool)
                .await?;
        }
        for (op, txout) in txouts {
            let OutPoint { txid, vout } = op;
            let TxOut {
                value,
                script_pubkey,
            } = txout;
            sqlx::query("INSERT INTO txout(txid, vout, value, script) VALUES($1, $2, $3, $4) ON CONFLICT DO UPDATE SET value = $3, script = $4")
                .bind(txid.to_string())
                .bind(vout)
                .bind(i64::try_from(value.to_sat())?)
                .bind(script_pubkey.to_bytes())
                .execute(&self.pool)
                .await?;
        }
        for (anchor, txid) in anchors {
            let BlockId { height, hash } = anchor.block_id;
            let confirmation_time = anchor.confirmation_time;
            sqlx::query("INSERT OR IGNORE INTO anchor(block_height, block_hash, txid, confirmation_time) VALUES($1, $2, $3, $4)")
                .bind(height)
                .bind(hash.to_string())
                .bind(txid.to_string())
                .bind(i64::try_from(confirmation_time)?)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }

    /// Write local_chain.
    pub async fn write_local_chain(
        &self,
        local_chain: &local_chain::ChangeSet,
    ) -> Result<(), Error> {
        for (&height, hash) in &local_chain.blocks {
            match hash {
                Some(hash) => {
                    // Avoid inserting new rows of existing height.
                    // FIXME: The correct way to handle this is to have a unique constraint on `height`
                    // in the block table schema.
                    let row_option = sqlx::query("SELECT height FROM block WHERE height = $1")
                        .bind(height)
                        .fetch_optional(&self.pool)
                        .await?;
                    if row_option.is_none() {
                        sqlx::query("INSERT OR IGNORE INTO block(height, hash) VALUES($1, $2)")
                            .bind(height)
                            .bind(hash.to_string())
                            .execute(&self.pool)
                            .await?;
                    }
                }
                None => {
                    sqlx::query("DELETE FROM block WHERE height = $1")
                        .bind(height)
                        .execute(&self.pool)
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Write keychain_txout.
    pub async fn write_keychain_txout(
        &self,
        keychain_txout: &keychain_txout::ChangeSet,
    ) -> Result<(), Error> {
        for (descriptor_id, last_revealed) in &keychain_txout.last_revealed {
            sqlx::query(
                "INSERT INTO keychain_last_revealed(descriptor_id, last_revealed) VALUES($1, $2) ON CONFLICT DO UPDATE SET last_revealed = $2",
            )
            .bind(descriptor_id.to_string())
            .bind(last_revealed)
            .execute(&self.pool)
            .await?;
        }
        for (descriptor_id, spk_cache) in &keychain_txout.spk_cache {
            for (derivation_index, script) in spk_cache {
                sqlx::query(
                    "INSERT OR IGNORE INTO keychain_script_pubkey(descriptor_id, derivation_index, script) VALUES($1, $2, $3)",
                )
                .bind(descriptor_id.to_string())
                .bind(*derivation_index)
                .bind(script.to_bytes())
                .execute(&self.pool)
                .await?;
            }
        }

        Ok(())
    }

    /// Read tx_graph.
    pub async fn read_tx_graph(&self) -> Result<tx_graph::ChangeSet<ConfirmationBlockTime>, Error> {
        let mut changeset = tx_graph::ChangeSet::default();

        let rows = sqlx::query("SELECT txid, tx, first_seen, last_seen, last_evicted FROM tx")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let txid: String = row.get("txid");
            let txid: Txid = txid.parse()?;
            let data: Vec<u8> = row.get("tx");
            let tx: Transaction = consensus::encode::deserialize(&data)?;
            let first_seen: i64 = row.get("first_seen");
            let last_seen: i64 = row.get("last_seen");
            let last_evicted: i64 = row.get("last_evicted");

            changeset.txs.insert(Arc::new(tx));
            changeset.first_seen.insert(txid, first_seen.try_into()?);
            changeset.last_seen.insert(txid, last_seen.try_into()?);
            changeset
                .last_evicted
                .insert(txid, last_evicted.try_into()?);
        }

        let rows = sqlx::query("SELECT txid, vout, value, script FROM txout")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let txid: String = row.get("txid");
            let txid: Txid = txid.parse()?;
            let vout: u32 = row.get("vout");
            let value: i64 = row.get("value");
            let value = Amount::from_sat(value.try_into()?);
            let script: Vec<u8> = row.get("script");
            let script_pubkey = ScriptBuf::from_bytes(script);
            let outpoint = OutPoint { txid, vout };
            let txout = TxOut {
                value,
                script_pubkey,
            };
            changeset.txouts.insert(outpoint, txout);
        }

        let rows =
            sqlx::query("SELECT block_height, block_hash, txid, confirmation_time FROM anchor")
                .fetch_all(&self.pool)
                .await?;
        for row in rows {
            let height: u32 = row.get("block_height");
            let hash: String = row.get("block_hash");
            let hash: BlockHash = hash.parse()?;
            let txid: String = row.get("txid");
            let txid: Txid = txid.parse()?;
            let confirmation_time: i64 = row.get("confirmation_time");
            let anchor = ConfirmationBlockTime {
                block_id: BlockId { height, hash },
                confirmation_time: confirmation_time.try_into()?,
            };
            changeset.anchors.insert((anchor, txid));
        }

        Ok(changeset)
    }

    /// Read local_chain.
    pub async fn read_local_chain(&self) -> Result<local_chain::ChangeSet, Error> {
        let mut changeset = local_chain::ChangeSet::default();

        let rows = sqlx::query("SELECT height, hash FROM block")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let height: u32 = row.get("height");
            let hash: String = row.get("hash");
            let hash: BlockHash = hash.parse()?;
            changeset.blocks.insert(height, Some(hash));
        }

        Ok(changeset)
    }

    /// Read keychain_txout.
    pub async fn read_keychain_txout(&self) -> Result<keychain_txout::ChangeSet, Error> {
        let mut changeset = keychain_txout::ChangeSet::default();

        let rows = sqlx::query("SELECT descriptor_id, last_revealed FROM keychain_last_revealed")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let descriptor_id: String = row.get("descriptor_id");
            let descriptor_id: DescriptorId = descriptor_id.parse()?;
            let last_revealed: u32 = row.get("last_revealed");
            changeset.last_revealed.insert(descriptor_id, last_revealed);
        }

        let rows = sqlx::query(
            "SELECT descriptor_id, derivation_index, script FROM keychain_script_pubkey",
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let descriptor_id: String = row.get("descriptor_id");
            let descriptor_id: DescriptorId = descriptor_id.parse()?;
            let derivation_index: u32 = row.get("derivation_index");
            let script: Vec<u8> = row.get("script");
            let script = ScriptBuf::from_bytes(script);
            changeset
                .spk_cache
                .entry(descriptor_id)
                .or_default()
                .insert(derivation_index, script);
        }

        Ok(changeset)
    }
}
