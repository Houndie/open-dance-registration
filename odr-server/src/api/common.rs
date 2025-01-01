use common::proto::{string_query, StringQuery};

use odr_core::store;

use super::ValidationError;

pub fn try_logical_string_query<F: store::Field<Item = String>>(
    q: StringQuery,
) -> Result<store::LogicalQuery<F>, ValidationError> {
    match q.operator {
        Some(string_query::Operator::Equals(equals)) => Ok(store::LogicalQuery::Equals(equals)),
        Some(string_query::Operator::NotEquals(not_equals)) => {
            Ok(store::LogicalQuery::NotEquals(not_equals))
        }
        None => Err(ValidationError::new_empty("operator")),
    }
}
