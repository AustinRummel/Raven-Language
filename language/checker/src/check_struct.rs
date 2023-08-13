use std::sync::Arc;
use indexmap::IndexMap;
use no_deadlocks::Mutex;
use syntax::ParsingError;
use syntax::code::{FinalizedField, FinalizedMemberField};
use syntax::r#struct::{FinalizedStruct, UnfinalizedStruct};
use syntax::syntax::Syntax;
use syntax::types::FinalizedTypes;
use crate::finalize_generics;
use crate::output::TypesChecker;

pub async fn verify_struct(_process_manager: &TypesChecker, structure: UnfinalizedStruct,
                           syntax: &Arc<Mutex<Syntax>>, include_refs: bool) -> Result<FinalizedStruct, ParsingError> {
    println!("Here: {}", structure.data.name);
    let mut finalized_fields = Vec::new();
    for field in structure.fields {
        let field = field.await?;
        let mut field_type = field.field.field_type.finalize(syntax.clone()).await;
        if include_refs {
            field_type = FinalizedTypes::Reference(Box::new(field_type));
        }
        finalized_fields.push(FinalizedMemberField { modifiers: field.modifiers, attributes: field.attributes,
            field: FinalizedField { field_type, name: field.field.name } })
    }

    let mut generics = IndexMap::new();

    finalize_generics(syntax, structure.generics, &mut generics).await?;

    return Ok(FinalizedStruct {
        generics,
        fields: finalized_fields,
        data: structure.data,
    });
}