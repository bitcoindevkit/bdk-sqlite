-- 0002_schema_up.sql

-- ******************************************** --
-- Change primary key of block table to height. --
-- ******************************************** --

-- Create new table
CREATE TABLE IF NOT EXISTS block_new(
    height INTEGER PRIMARY KEY NOT NULL,
    hash TEXT NOT NULL
);
-- Copy only unique rows
INSERT INTO block_new(height, hash)
SELECT height, hash
FROM block
WHERE height IN(
    SELECT height
    FROM block
    GROUP BY height
    HAVING COUNT(*) = 1
);
-- Drop old table
DROP TABLE block;
-- Rename new table to old
ALTER TABLE block_new RENAME TO block;

-- ************************************************ --
-- Change type of anchor block_hash column to TEXT. --
-- ************************************************ --

-- Create new table
CREATE TABLE IF NOT EXISTS anchor_new(
    block_height INTEGER NOT NULL,
    block_hash TEXT NOT NULL,
    txid TEXT NOT NULL,
    confirmation_time INTEGER NOT NULL,
    PRIMARY KEY(block_height, block_hash, txid)
);
-- Copy old data
INSERT INTO anchor_new(block_height, block_hash, txid, confirmation_time)
SELECT block_height, block_hash, txid, confirmation_time
FROM anchor;
-- Delete old table
DROP TABLE anchor;
-- Rename new table to old table
ALTER TABLE anchor_new RENAME TO anchor;
