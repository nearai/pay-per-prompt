-- Add migration script here
CREATE TABLE IF NOT EXISTS channel (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name VARCHAR(255) NOT NULL UNIQUE,
    sender VARCHAR(255) NOT NULL,
    sender_pk VARCHAR(255) NOT NULL,
    receiver VARCHAR(255) NOT NULL,
    receiver_pk VARCHAR(255) NOT NULL,
    added_balance BLOB NOT NULL CHECK (length(added_balance) = 16),
    withdraw_balance BLOB NOT NULL CHECK (length(withdraw_balance) = 16)
);

CREATE TABLE IF NOT EXISTS signed_state (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    channel_id INT NOT NULL,
    spent_balance BLOB NOT NULL CHECK (length(spent_balance) = 16),
    signature TEXT NOT NULL,
    FOREIGN KEY (channel_id) REFERENCES channel(id)
);