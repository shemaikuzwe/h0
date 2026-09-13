// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "vm_status"))]
    pub struct VmStatus;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::VmStatus;

    vms (id) {
        id -> Int4,
        #[max_length = 255]
        name -> Varchar,
        cpu -> Int4,
        memory -> Int4,
        status -> VmStatus,
        created_at -> Nullable<Timestamp>,
        updated_at -> Nullable<Timestamp>,
    }
}
