-- Add locked outpoints table

CREATE TABLE IF NOT EXISTS locked_outpoint(
    txid TEXT NOT NULL,
    vout INTEGER NOT NULL,
    PRIMARY KEY(txid, vout)
);
