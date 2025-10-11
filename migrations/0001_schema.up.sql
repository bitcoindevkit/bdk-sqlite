DROP TABLE IF EXISTS block;
DROP TABLE IF EXISTS tx;
DROP TABLE IF EXISTS txout;
DROP TABLE IF EXISTS anchor;
DROP TABLE IF EXISTS keychain_last_revealed;
DROP TABLE IF EXISTS keychain_script_pubkey;
DROP TABLE IF EXISTS keychain;
DROP TABLE IF EXISTS network;

-- Block table
CREATE TABLE IF NOT EXISTS block(
    height INTEGER NOT NULL,
    hash TEXT NOT NULL,
    PRIMARY KEY(height, hash)
);

-- Transaction table
CREATE TABLE IF NOT EXISTS tx(
    txid TEXT NOT NULL,
    tx BLOB,
    first_seen INTEGER,
    last_seen INTEGER,
    last_evicted INTEGER,
    PRIMARY KEY(txid)
);

-- TxOut table
CREATE TABLE IF NOT EXISTS txout(
    txid TEXT NOT NULL,
    vout INTEGER NOT NULL,
    value INTEGER NOT NULL,
    script BLOB NOT NULL,
    PRIMARY KEY(txid, vout)
);

-- Anchor table
CREATE TABLE IF NOT EXISTS anchor(
    block_height INTEGER NOT NULL,
    block_hash INTEGER NOT NULL,
    txid TEXT NOT NULL,
    confirmation_time INTEGER NOT NULL,
    PRIMARY KEY(block_height, block_hash, txid)
);

-- Keychain last revealed table
CREATE TABLE IF NOT EXISTS keychain_last_revealed(
    descriptor_id TEXT NOT NULL,
    last_revealed INTEGER,
    PRIMARY KEY(descriptor_id)
);

-- Keychain script pubkey table
CREATE TABLE IF NOT EXISTS keychain_script_pubkey(
    descriptor_id TEXT NOT NULL,
    derivation_index INTEGER,
    script BLOB,
    PRIMARY KEY(descriptor_id, derivation_index)
);

-- Keychain (descriptor) table
CREATE TABLE IF NOT EXISTS keychain(
    keychain INTEGER NOT NULL,
    descriptor TEXT NOT NULL,
    PRIMARY KEY(keychain)
);

-- Network table
CREATE TABLE IF NOT EXISTS network(
    network TEXT NOT NULL
);
