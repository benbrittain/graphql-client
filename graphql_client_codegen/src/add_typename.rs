/// Transforms a GraphQL query document to add `__typename` to all selection sets.
/// This matches Apollo's behavior for persisted query hash computation.
use graphql_parser::query::{
    Definition, Document, Field, FragmentDefinition, InlineFragment, OperationDefinition,
    Selection, SelectionSet,
};

/// Adds `__typename` to all selection sets in the document.
pub fn add_typename_to_document<'a>(
    doc: &Document<'a, String>,
) -> Document<'a, String> {
    Document {
        definitions: doc
            .definitions
            .iter()
            .map(add_typename_to_definition)
            .collect(),
    }
}

fn add_typename_to_definition<'a>(
    def: &Definition<'a, String>,
) -> Definition<'a, String> {
    match def {
        Definition::Operation(op) => {
            Definition::Operation(add_typename_to_operation(op))
        }
        Definition::Fragment(frag) => {
            Definition::Fragment(add_typename_to_fragment(frag))
        }
    }
}

fn add_typename_to_operation<'a>(
    op: &OperationDefinition<'a, String>,
) -> OperationDefinition<'a, String> {
    // Don't add __typename at the root operation level, only in nested selection sets
    match op {
        OperationDefinition::Query(q) => {
            OperationDefinition::Query(graphql_parser::query::Query {
                position: q.position,
                name: q.name.clone(),
                variable_definitions: q.variable_definitions.clone(),
                directives: q.directives.clone(),
                selection_set: add_typename_to_selection_set_no_root(&q.selection_set),
            })
        }
        OperationDefinition::Mutation(m) => {
            OperationDefinition::Mutation(graphql_parser::query::Mutation {
                position: m.position,
                name: m.name.clone(),
                variable_definitions: m.variable_definitions.clone(),
                directives: m.directives.clone(),
                selection_set: add_typename_to_selection_set_no_root(&m.selection_set),
            })
        }
        OperationDefinition::Subscription(s) => {
            OperationDefinition::Subscription(graphql_parser::query::Subscription {
                position: s.position,
                name: s.name.clone(),
                variable_definitions: s.variable_definitions.clone(),
                directives: s.directives.clone(),
                selection_set: add_typename_to_selection_set_no_root(&s.selection_set),
            })
        }
        OperationDefinition::SelectionSet(ss) => {
            OperationDefinition::SelectionSet(add_typename_to_selection_set_no_root(ss))
        }
    }
}

/// Process selection set items but don't add __typename at this level (for root operations)
fn add_typename_to_selection_set_no_root<'a>(
    ss: &SelectionSet<'a, String>,
) -> SelectionSet<'a, String> {
    let items: Vec<Selection<'a, String>> = ss
        .items
        .iter()
        .map(add_typename_to_selection)
        .collect();

    SelectionSet {
        span: ss.span,
        items,
    }
}

fn add_typename_to_fragment<'a>(
    frag: &FragmentDefinition<'a, String>,
) -> FragmentDefinition<'a, String> {
    FragmentDefinition {
        position: frag.position,
        name: frag.name.clone(),
        type_condition: frag.type_condition.clone(),
        directives: frag.directives.clone(),
        selection_set: add_typename_to_selection_set(&frag.selection_set),
    }
}

fn add_typename_to_selection_set<'a>(
    ss: &SelectionSet<'a, String>,
) -> SelectionSet<'a, String> {
    let mut items: Vec<Selection<'a, String>> = ss
        .items
        .iter()
        .map(add_typename_to_selection)
        .collect();

    // Add __typename at the end if not already present and there are fields
    let has_typename = items.iter().any(|item| {
        matches!(item, Selection::Field(f) if f.name == "__typename")
    });

    if !has_typename && !items.is_empty() {
        items.push(Selection::Field(Field {
            position: graphql_parser::Pos { line: 0, column: 0 },
            alias: None,
            name: "__typename".to_string(),
            arguments: vec![],
            directives: vec![],
            selection_set: SelectionSet {
                span: (graphql_parser::Pos { line: 0, column: 0 }, graphql_parser::Pos { line: 0, column: 0 }),
                items: vec![],
            },
        }));
    }

    SelectionSet {
        span: ss.span,
        items,
    }
}

fn add_typename_to_selection<'a>(
    sel: &Selection<'a, String>,
) -> Selection<'a, String> {
    match sel {
        Selection::Field(f) => Selection::Field(add_typename_to_field(f)),
        Selection::FragmentSpread(fs) => Selection::FragmentSpread(fs.clone()),
        Selection::InlineFragment(inf) => {
            Selection::InlineFragment(add_typename_to_inline_fragment(inf))
        }
    }
}

fn add_typename_to_field<'a>(field: &Field<'a, String>) -> Field<'a, String> {
    Field {
        position: field.position,
        alias: field.alias.clone(),
        name: field.name.clone(),
        arguments: field.arguments.clone(),
        directives: field.directives.clone(),
        selection_set: add_typename_to_selection_set(&field.selection_set),
    }
}

fn add_typename_to_inline_fragment<'a>(
    inf: &InlineFragment<'a, String>,
) -> InlineFragment<'a, String> {
    InlineFragment {
        position: inf.position,
        type_condition: inf.type_condition.clone(),
        directives: inf.directives.clone(),
        selection_set: add_typename_to_selection_set(&inf.selection_set),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn test_adds_typename_to_simple_query() {
        let query = r#"
            query GetDeviceToken($session: UUID!) {
                deviceToken(session: $session) {
                    encryptedToken
                    serverPubKey
                    iv
                }
            }
        "#;

        let doc = graphql_parser::parse_query::<String>(query).unwrap();
        let transformed = add_typename_to_document(&doc);
        let output = format!("{}", transformed).trim_end().to_string();

        eprintln!("Transformed query:\n{}", output);
        eprintln!("---");
        eprintln!("Bytes: {:?}", output.as_bytes());

        let mut hasher = Sha256::new();
        hasher.update(output.as_bytes());
        let hash_bytes = hasher.finalize();
        let hash = format!("{:x}", hash_bytes);
        eprintln!("Hash: {}", hash);

        assert!(output.contains("__typename"), "Output should contain __typename: {}", output);
    }
}
