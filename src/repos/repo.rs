use std::fmt::Debug;

use diesel::associations::HasTable;
use diesel::expression::operators::Eq;
use diesel::expression::AsExpression;
use diesel::helper_types::Filter;
use diesel::pg::{Pg, PgConnection};
use diesel::query_builder::{AsChangeset, InsertStatement, IntoUpdateTarget, QueryFragment, QueryId};
use diesel::query_dsl::methods::FilterDsl;
use diesel::query_dsl::LoadQuery;
use diesel::sql_types::HasSqlType;
use diesel::{Expression, Insertable, OptionalExtension, QuerySource, Queryable, RunQueryDsl, Table};
use failure::Fail;

use super::{ErrorKind, RepoResult};

pub trait Select<Tbl, Expr, Filtered, Record>
where
    Expr: diesel::ExpressionMethods,
    Filtered: AsExpression<<Expr as Expression>::SqlType> + std::fmt::Debug + Clone,
    Tbl: FilterDsl<Eq<Expr, <Filtered as AsExpression<<Expr as diesel::Expression>::SqlType>>::Expression>>,
    Filter<Tbl, Eq<Expr, <Filtered as AsExpression<<Expr as diesel::Expression>::SqlType>>::Expression>>: LoadQuery<PgConnection, Record>,
{
    fn get(conn: &PgConnection, table: Tbl, expr: Expr, filter: Filtered) -> RepoResult<Option<Record>> {
        let query = table.filter(expr.eq(filter.clone()));
        query.get_result(conn).optional().map_err(ectx!(ErrorKind::Internal => filter))
    }
}

pub trait Insert<Tbl, Payload, Record>
where
    Payload: Insertable<Tbl> + std::fmt::Debug + Clone,
    InsertStatement<Tbl, Payload::Values>: LoadQuery<PgConnection, Record>,
{
    fn create(conn: &PgConnection, table: Tbl, payload: Payload) -> RepoResult<Record> {
        diesel::insert_into(table)
            .values(payload.clone())
            .get_result::<Record>(conn)
            .map_err(ectx!(ErrorKind::Internal => payload))
    }
}

pub trait Update<Tbl, Expr, Filtered, Record, Payload>
where
    Expr: diesel::ExpressionMethods,
    Filtered: AsExpression<<Expr as Expression>::SqlType> + std::fmt::Debug + Clone,
    Payload: AsChangeset<Target = Tbl> + std::fmt::Debug + Clone,
    <Payload as AsChangeset>::Changeset: QueryFragment<Pg>,
    Tbl: FilterDsl<Eq<Expr, <Filtered as AsExpression<<Expr as diesel::Expression>::SqlType>>::Expression>>
        + HasTable
        + Table,
    Filter<Tbl, Eq<Expr, <Filtered as AsExpression<<Expr as diesel::Expression>::SqlType>>::Expression>>:
        HasTable + IntoUpdateTarget<Table = Tbl>,
    <Tbl as QuerySource>::FromClause: QueryFragment<Pg>,
    <Tbl as Table>::AllColumns: QueryFragment<Pg>,
    Pg: HasSqlType<<<Tbl as Table>::AllColumns as Expression>::SqlType>,
    Record: Queryable<<<Tbl as Table>::AllColumns as Expression>::SqlType, Pg>,
    <<Tbl as FilterDsl<
        Eq<
            Expr,
            <Filtered as AsExpression<<Expr as Expression>::SqlType>>::Expression,
        >,
    >>::Output as IntoUpdateTarget>::WhereClause: QueryFragment<Pg>,
{
    fn update(conn: &PgConnection, table: Tbl, expr: Expr, filter: Filtered, payload: Payload) -> RepoResult<Record> {
        let f = table.filter(expr.eq(filter.clone()));
        diesel::update(f)
            .set(payload.clone())
            .get_result(conn)
            .map_err(ectx!(ErrorKind::Internal => filter, payload))
    }
}

pub trait Delete<Tbl, Expr, Filtered, Record>
where
    Expr: diesel::ExpressionMethods,
    Filtered: AsExpression<<Expr as Expression>::SqlType> + Debug + Clone,
    Tbl: FilterDsl<Eq<Expr, <Filtered as AsExpression<<Expr as Expression>::SqlType>>::Expression>>
        + HasTable
        + Table
        + QueryId,
    Filter<Tbl, Eq<Expr, <Filtered as AsExpression<<Expr as Expression>::SqlType>>::Expression>>:
        HasTable + IntoUpdateTarget<Table = Tbl>,
    <Tbl as QuerySource>::FromClause: QueryFragment<Pg>,
    <Tbl as Table>::AllColumns: QueryFragment<Pg>,
    Pg: HasSqlType<<<Tbl as Table>::AllColumns as Expression>::SqlType>,
    Record: Queryable<<<Tbl as Table>::AllColumns as Expression>::SqlType, Pg>,
    <<Tbl as FilterDsl<
        Eq<
            Expr,
            <Filtered as AsExpression<<Expr as Expression>::SqlType>>::Expression,
        >,
    >>::Output as IntoUpdateTarget>::WhereClause: QueryFragment<Pg> + QueryId,
    <Tbl as Table>::AllColumns: QueryId,

{
    fn delete(conn: &PgConnection, table: Tbl, expr: Expr, filter: Filtered) -> RepoResult<Record> {
        let filtered = table.filter(expr.eq(filter.clone()));
        diesel::delete(filtered)
            .get_result(conn)
            .map_err(ectx!(ErrorKind::Internal => filter))
    }
}

pub mod test {
    use models::*;
    use repos::repo::*;
    use schema::users::dsl as Users;

    pub trait UsersRepo2:
        Select<Users::users, Users::id, UserId, User>
        + Select<Users::users, Users::authentication_token, AuthenticationToken, User>
        + Insert<Users::users, NewUser, User>
        + Update<Users::users, Users::id, UserId, User, UpdateUser>
        + Delete<Users::users, Users::id, UserId, User>
    {
}
}
