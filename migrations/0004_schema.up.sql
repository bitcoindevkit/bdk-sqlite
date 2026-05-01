-- Recreate the network table with an `id` column that is both the PRIMARY KEY and
-- always checked to be 0. This makes it structurally impossible for more
-- than one row to exist.

-- ********************************************** --
-- Add singleton constraint to the network table. --
-- ********************************************** --

CREATE TABLE IF NOT EXISTS network_new(
    id      INTEGER NOT NULL DEFAULT 0 CHECK(id = 0),
    network TEXT    NOT NULL,
    PRIMARY KEY(id)
);

-- Copy at most one existing row, pinning id to 0.
INSERT OR IGNORE INTO network_new(id, network)
SELECT 0, network FROM network LIMIT 1;

DROP TABLE network;

ALTER TABLE network_new RENAME TO network;
