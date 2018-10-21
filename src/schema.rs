table! {
    accounts (id) {
        id -> Uuid,
        user_id -> Uuid,
        currency -> Varchar,
        address -> Varchar,
        name -> Nullable<Varchar>,
        kind -> Varchar,
        balance -> Numeric,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    blockchain_transactions (hash) {
        hash -> Varchar,
        from_ -> Varchar,
        to_ -> Varchar,
        block_number -> Int8,
        currency -> Varchar,
        value -> Numeric,
        fee -> Numeric,
        confirmations -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

table! {
    seen_hashes (hash) {
        hash -> Varchar,
        block_number -> Int8,
        currency -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
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
        hold_until -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
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
    seen_hashes,
    transactions,
    users,
);
