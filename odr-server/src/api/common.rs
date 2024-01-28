use common::proto::{string_query, StringQuery};

use crate::store;

use super::ValidationError;

pub fn validate_string_query(query: &StringQuery) -> Result<(), ValidationError> {
    if query.operator.is_none() {
        return Err(ValidationError::new_empty("operator"));
    }

    Ok(())
}

pub fn to_logical_string_query<F: store::Field<Item = String>>(
    q: StringQuery,
) -> store::LogicalQuery<F> {
    match q.operator.unwrap() {
        string_query::Operator::Equals(equals) => store::LogicalQuery::Equals(equals),
        string_query::Operator::NotEquals(not_equals) => store::LogicalQuery::NotEquals(not_equals),
    }
}
