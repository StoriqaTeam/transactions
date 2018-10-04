table! {
    users (id) {
        id -> Uuid,
        name -> Varchar,
        authentication_token -> Varchar,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}
