table! {
    accounts (id) {
        id -> Uuid,
        user_id -> Uuid,
        balance -> Numeric,
        currency -> Varchar,
        account_address -> Varchar,
        name -> Varchar,
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

allow_tables_to_appear_in_same_query!(accounts, users,);
