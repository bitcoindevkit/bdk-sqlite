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
pub struct Store {
    /// Pool.
    pub(crate) pool: Pool,
}

impl Store {
    /// New in memory.
    pub async fn new_memory() -> Result<Self, Error> {
        let pool = Pool::connect("sqlite::memory:").await?;

        Ok(Self { pool })
    }

    /// Open a new [`Store`] instance.
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

    /// Migrate.
    pub(crate) async fn migrate(&self) -> Result<(), Error> {
        Ok(sqlx::migrate!().run(&self.pool).await?)
    }
}

impl Store {
    /// Write tx_graph.
    pub async fn write_tx_graph(
        &self,
        tx_graph: &tx_graph::ChangeSet<ConfirmationBlockTime>,
    ) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        let txs = &tx_graph.txs;
        let txouts = &tx_graph.txouts;
        let anchors = &tx_graph.anchors;
        let first_seen = &tx_graph.first_seen;
        let last_seen = &tx_graph.last_seen;
        let last_evicted = &tx_graph.last_evicted;

        for tx in txs {
            let txid = tx.compute_txid();
            sqlx::query("insert into tx(txid, tx) values($1, $2)")
                .bind(txid.to_string())
                .bind(consensus::encode::serialize(tx))
                .execute(&mut *conn)
                .await?;
        }
        for (txid, t) in first_seen {
            sqlx::query("insert into tx(txid, first_seen) values($1, $2) on conflict do update set first_seen = $2")
                .bind(txid.to_string())
                .bind(i64::try_from(*t)?)
                .execute(&mut *conn)
                .await?;
        }
        for (txid, t) in last_seen {
            sqlx::query("insert into tx(txid, last_seen) values($1, $2) on conflict do update set last_seen = $2")
                .bind(txid.to_string())
                .bind(i64::try_from(*t)?)
                .execute(&mut *conn)
                .await?;
        }
        for (txid, t) in last_evicted {
            sqlx::query("insert into tx(txid, last_evicted) values($1, $2) on conflict do update set last_evicted = $2")
                .bind(txid.to_string())
                .bind(i64::try_from(*t)?)
                .execute(&mut *conn)
                .await?;
        }
        for (op, txout) in txouts {
            let OutPoint { txid, vout } = op;
            let TxOut {
                value,
                script_pubkey,
            } = txout;
            sqlx::query("insert into txout(txid, vout, value, script) values($1, $2, $3, $4)")
                .bind(txid.to_string())
                .bind(vout)
                .bind(i64::try_from(value.to_sat())?)
                .bind(script_pubkey.to_bytes())
                .execute(&mut *conn)
                .await?;
        }
        for (anchor, txid) in anchors {
            let BlockId { height, hash } = anchor.block_id;
            let confirmation_time = anchor.confirmation_time;
            sqlx::query("insert into anchor(block_height, block_hash, txid, confirmation_time) values($1, $2, $3, $4)")
                .bind(height)
                .bind(hash.to_string())
                .bind(txid.to_string())
                .bind(i64::try_from(confirmation_time)?)
                .execute(&mut *conn)
                .await?;
        }

        Ok(())
    }

    /// Write local_chain.
    pub async fn write_local_chain(
        &self,
        local_chain: &local_chain::ChangeSet,
    ) -> Result<(), Error> {
        let mut conn = self.pool.acquire().await?;

        for (&height, hash) in &local_chain.blocks {
            match hash {
                Some(hash) => {
                    sqlx::query("insert or replace into block(height, hash) values($1, $2)")
                        .bind(height)
                        .bind(hash.to_string())
                        .execute(&mut *conn)
                        .await?;
                }
                None => {
                    sqlx::query("delete from block where height = $1")
                        .bind(height)
                        .execute(&mut *conn)
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
        let mut conn = self.pool.acquire().await?;

        for (descriptor_id, last_revealed) in &keychain_txout.last_revealed {
            sqlx::query(
                "insert or replace into keychain_last_revealed(descriptor_id, last_revealed) values($1, $2)",
            )
            .bind(descriptor_id.to_string())
            .bind(last_revealed)
            .execute(&mut *conn)
            .await?;
        }
        for (descriptor_id, spk_cache) in &keychain_txout.spk_cache {
            for (derivation_index, script) in spk_cache {
                sqlx::query(
                    "insert or replace into keychain_script_pubkey(descriptor_id, derivation_index, script) values($1, $2, $3)",
                )
                .bind(descriptor_id.to_string())
                .bind(*derivation_index)
                .bind(script.to_bytes())
                .execute(&mut *conn)
                .await?;
            }
        }

        Ok(())
    }

    /// Read tx_graph.
    pub async fn read_tx_graph(&self) -> Result<tx_graph::ChangeSet<ConfirmationBlockTime>, Error> {
        let mut conn = self.pool.acquire().await?;

        let mut changeset = tx_graph::ChangeSet::default();

        let rows = sqlx::query("select txid, tx, first_seen, last_seen, last_evicted from tx")
            .fetch_all(&mut *conn)
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
            changeset.last_seen.insert(txid, last_evicted.try_into()?);
        }

        let rows = sqlx::query("SELECT txid, vout, value, script FROM txout")
            .fetch_all(&mut *conn)
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
            sqlx::query("select block_height, block_hash, txid, confirmation_time from anchor")
                .fetch_all(&mut *conn)
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
        let mut conn = self.pool.acquire().await?;

        let mut changeset = local_chain::ChangeSet::default();

        let rows = sqlx::query("select height, hash from block")
            .fetch_all(&mut *conn)
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
        let mut conn = self.pool.acquire().await?;

        let mut changeset = keychain_txout::ChangeSet::default();

        let rows = sqlx::query("select descriptor_id, last_revealed from keychain_last_revealed")
            .fetch_all(&mut *conn)
            .await?;
        for row in rows {
            let descriptor_id: String = row.get("descriptor_id");
            let descriptor_id: DescriptorId = descriptor_id.parse()?;
            let last_revealed: u32 = row.get("last_revealed");
            changeset.last_revealed.insert(descriptor_id, last_revealed);
        }

        let rows = sqlx::query(
            "select descriptor_id, derivation_index, script from keychain_script_pubkey",
        )
        .fetch_all(&mut *conn)
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
