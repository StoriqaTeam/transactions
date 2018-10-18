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

allow_tables_to_appear_in_same_query!(accounts, transactions, users,);
