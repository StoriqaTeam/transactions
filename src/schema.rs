table! {
    accounts (id) {
        id -> Uuid,
        user_id -> Uuid,
        currency -> Varchar,
        address -> Varchar,
        name -> Nullable<Varchar>,
        kind -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        erc20_approved -> Bool,
    }
}

table! {
    blockchain_transactions (hash) {
        hash -> Varchar,
        block_number -> Int8,
        currency -> Varchar,
        fee -> Numeric,
        confirmations -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        from_ -> Jsonb,
        to_ -> Jsonb,
        erc20_operation_kind -> Nullable<Varchar>,
    }
}

table! {
    key_values (key) {
        key -> Varchar,
        value -> Jsonb,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    pending_blockchain_transactions (hash) {
        hash -> Varchar,
        from_ -> Varchar,
        to_ -> Varchar,
        currency -> Varchar,
        value -> Numeric,
        fee -> Numeric,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        erc20_operation_kind -> Nullable<Varchar>,
    }
}

table! {
    seen_hashes (hash, currency) {
        hash -> Varchar,
        block_number -> Int8,
        currency -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    strange_blockchain_transactions (hash) {
        hash -> Varchar,
        from_ -> Jsonb,
        to_ -> Jsonb,
        block_number -> Int8,
        currency -> Varchar,
        fee -> Numeric,
        confirmations -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        commentary -> Varchar,
        erc20_operation_kind -> Nullable<Varchar>,
    }
}

table! {
    transactions (id) {
        id -> Uuid,
        user_id -> Uuid,
        dr_account_id -> Uuid,
        cr_account_id -> Uuid,
        currency -> Varchar,
        value -> Numeric,
        status -> Varchar,
        blockchain_tx_id -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        gid -> Uuid,
        kind -> Varchar,
        group_kind -> Varchar,
        related_tx -> Nullable<Uuid>,
        meta -> Jsonb,
    }
}

table! {
    users (id) {
        id -> Uuid,
        name -> Varchar,
        authentication_token -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

joinable!(accounts -> users (user_id));
joinable!(transactions -> users (user_id));

allow_tables_to_appear_in_same_query!(
    accounts,
    blockchain_transactions,
    key_values,
    pending_blockchain_transactions,
    seen_hashes,
    strange_blockchain_transactions,
    transactions,
    users,
);
