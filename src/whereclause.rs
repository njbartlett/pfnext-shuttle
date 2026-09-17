use sqlx::{Database, Encode, QueryBuilder, Type};

#[derive(Debug)]
pub enum Operator {
    Equal,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    /// Case-insensitive pattern match (Postgres `ILIKE`). Use [`like_contains`] to build a
    /// safe "contains" pattern from user input.
    ILike
}

impl Operator {
    fn to_sql(&self) -> String {
        match self {
            Operator::Equal => " = ",
            Operator::LessThan => " < ",
            Operator::LessThanOrEqual => " <= ",
            Operator::GreaterThan => " > ",
            Operator::GreaterThanOrEqual => " >= ",
            Operator::ILike => " ILIKE ",
        }.to_string()
    }
}

/// Build a `LIKE`/`ILIKE` pattern matching any value that contains `term`, escaping the
/// pattern metacharacters (`%`, `_` and the default escape character `\`) so that they
/// match literally.
pub fn like_contains(term: &str) -> String {
    let mut pattern = String::with_capacity(term.len() + 2);
    pattern.push('%');
    for c in term.chars() {
        if c == '\\' || c == '%' || c == '_' {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

pub struct WhereClause {
    first: bool
}  

impl WhereClause {
    
    const WHERE_OP: &str = " WHERE ";
    const AND_OP: &str = " AND ";
    
    pub fn init() -> Self {
        Self { first: true }
    }

    pub fn append_to<'args, DB, T>(
        &mut self,
        qb: &mut QueryBuilder<'args, DB>,
        field_name: &str,
        operator: Operator,
        field_value: T
    ) -> &mut Self
    where
        DB: Database,
        T: 'args + Encode<'args, DB> + Type<DB>
    {
        qb.push(if self.first { Self::WHERE_OP } else { Self::AND_OP });
        qb.push(field_name);
        qb.push(operator.to_sql());
        qb.push_bind(field_value);

        self.first = false;
        self
    }

    pub fn opt_append_to<'args, DB, T>(
        &mut self,
        qb: &mut QueryBuilder<'args, DB>,
        field_name: &str,
        operator: Operator,
        opt_field_value: Option<T>
    ) -> &mut Self
    where
        DB: Database,
        T: 'args + Encode<'args, DB> + Type<DB>
    {
        if let Some(field_value) = opt_field_value {
            self.append_to(qb, field_name, operator, field_value)
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {

    use sqlx::{Execute, Postgres, QueryBuilder};

    use crate::whereclause::{like_contains, Operator, WhereClause};

    #[test]
    fn test_like_contains() {
        assert_eq!("%joe%", like_contains("joe"));
        assert_eq!("%%", like_contains(""));
        assert_eq!("%100\\%\\_off\\\\%", like_contains("100%_off\\"));
    }

    #[test]
    fn test_ilike_append() {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM things");
        let mut wc = WhereClause::init();

        wc.append_to(&mut qb, "name", Operator::ILike, like_contains("thing"));

        assert_eq!("SELECT * FROM things WHERE name ILIKE $1", qb.build().sql());
    }

    #[test]
    fn test_one_appends() {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM things");
        let mut wc = WhereClause::init();

        wc.append_to(&mut qb, "id", Operator::Equal, 0);

        assert_eq!("SELECT * FROM things WHERE id = $1", qb.build().sql());
    }

    #[test]
    fn test_multiple_appends() {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM things");
        let mut wc = WhereClause::init();

        wc.append_to(&mut qb, "id", Operator::Equal, 0);
        wc.append_to(&mut qb, "name", Operator::Equal, "thingy");
        wc.append_to(&mut qb, "size", Operator::GreaterThan, 10);

        assert_eq!("SELECT * FROM things WHERE id = $1 AND name = $2 AND size > $3", qb.build().sql());
    }

        #[test]
    fn test_opt_appends() {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM things");
        let mut wc = WhereClause::init();

        wc.opt_append_to(&mut qb, "id", Operator::Equal, Some(0));
        wc.opt_append_to(&mut qb, "name", Operator::Equal, None::<Option<String>>);
        
        assert_eq!("SELECT * FROM things WHERE id = $1", qb.build().sql());
    }

}