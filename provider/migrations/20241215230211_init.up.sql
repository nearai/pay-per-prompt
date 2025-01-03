-- Add migration script here
CREATE TABLE IF NOT EXISTS channel (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    name VARCHAR(255) NOT NULL UNIQUE,
    sender VARCHAR(255) NOT NULL,
    sender_pk VARCHAR(255) NOT NULL,
    receiver VARCHAR(255) NOT NULL,
    receiver_pk VARCHAR(255) NOT NULL,
    added_balance BLOB NOT NULL CHECK (length(added_balance) = 16),
    withdrawn_balance BLOB NOT NULL CHECK (length(withdrawn_balance) = 16),
    force_close_started TIMESTAMP DEFAULT NULL,
    soft_closed BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS signed_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    channel_id INT NOT NULL,
    spent_balance BLOB NOT NULL CHECK (length(spent_balance) = 16),
    signature TEXT NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES channel(id)
);