use crate::check_code::verify_effect;
use crate::check_method_call::check_method;
use crate::CodeVerifier;
use data::tokens::Span;
use std::mem;
use syntax::async_util::{AsyncDataGetter, UnparsedType};
use syntax::code::{degeneric_header, EffectType, Effects, FinalizedEffects};
use syntax::function::CodelessFinalizedFunction;
use syntax::r#struct::VOID;
use syntax::syntax::Syntax;
use syntax::top_element_manager::ImplWaiter;
use syntax::types::FinalizedTypes;
use syntax::ProcessManager;
use syntax::{ParsingError, SimpleVariableManager};

/// Checks an implementation call generated by control_parser or an operator to get the correct method
pub async fn check_impl_call(
    code_verifier: &mut CodeVerifier<'_>,
    variables: &mut SimpleVariableManager,
    effect: Effects,
) -> Result<FinalizedEffects, ParsingError> {
    let mut finalized_effects = Vec::default();
    let calling;
    let traits;
    let method;
    let returning;
    if let EffectType::ImplementationCall(new_calling, new_traits, new_method, effects, new_returning) = effect.types {
        for effect in effects {
            finalized_effects.push(verify_effect(code_verifier, variables, effect).await?)
        }
        calling = new_calling;
        traits = new_traits;
        method = new_method;
        returning = new_returning;
    } else {
        unreachable!()
    }

    let mut finding_return_type;
    if matches!(*calling, EffectType::NOP) {
        finding_return_type = FinalizedTypes::Struct(VOID.clone());
    } else {
        let found = verify_effect(code_verifier, variables, *calling).await?;
        finding_return_type = found.get_return(variables).unwrap();
        finding_return_type.fix_generics(code_verifier.process_manager, &code_verifier.syntax).await?;
        finalized_effects.insert(0, found);
    }

    if let Ok(inner) = Syntax::get_struct(
        code_verifier.syntax.clone(),
        ParsingError::empty(),
        traits.clone(),
        code_verifier.resolver.boxed_clone(),
        vec![],
    )
    .await
    {
        let data = inner.finalize(code_verifier.syntax.clone()).await;

        let mut impl_checker = ImplCheckerData {
            code_verifier,
            data: &data,
            returning: &returning,
            method: &method,
            finding_return_type: &finding_return_type,
            finalized_effects: &mut finalized_effects,
            variables,
        };
        if let Some(found) = check_virtual_type(&mut impl_checker, &effect.span).await? {
            return Ok(found);
        }

        let mut output = None;
        while output.is_none() && !code_verifier.syntax.lock().unwrap().finished_impls() {
            output = try_get_impl(&impl_checker, &effect.span).await?;
        }

        if output.is_none() {
            output = try_get_impl(&impl_checker, &effect.span).await?;
        }

        if output.is_none() {
            panic!("Failed for {} and {}", finding_return_type, data);
        }
        return Ok(output.unwrap());
    } else {
        panic!("Screwed up trait! {} for {:?}", traits, code_verifier.resolver.imports());
    }
}

/// All the data used by implementation checkers
pub struct ImplCheckerData<'a> {
    code_verifier: &'a CodeVerifier<'a>,
    data: &'a FinalizedTypes,
    returning: &'a Option<UnparsedType>,
    method: &'a String,
    finding_return_type: &'a FinalizedTypes,
    finalized_effects: &'a mut Vec<FinalizedEffects>,
    variables: &'a SimpleVariableManager,
}

/// Checks an implementation call to see if it should be a virtual call
async fn check_virtual_type(data: &mut ImplCheckerData<'_>, token: &Span) -> Result<Option<FinalizedEffects>, ParsingError> {
    if data.finding_return_type.of_type_sync(data.data, None).0 {
        let mut i = 0;
        for found in &data.data.inner_struct().data.functions {
            if found.name == *data.method {
                let mut temp = vec![];
                mem::swap(&mut temp, data.finalized_effects);

                let output = CodelessFinalizedFunction::degeneric(
                    AsyncDataGetter::new(data.code_verifier.syntax.clone(), found.clone()).await,
                    Box::new(data.code_verifier.process_manager.clone()),
                    &temp,
                    &data.code_verifier.syntax,
                    data.variables,
                    None,
                )
                .await?;
                return Ok(Some(FinalizedEffectType::VirtualCall(i, output, temp)));
            } else if found.name.split("::").last().unwrap() == data.method {
                let mut target = data.finding_return_type.find_method(&data.method).unwrap();
                if target.len() > 1 {
                    return Err(token.make_error("Ambiguous function!"));
                } else if target.is_empty() {
                    return Err(token.make_error("Unknown function!"));
                }
                let (_, target) = target.pop().unwrap();

                let return_type = data.finalized_effects[0].get_return(data.variables).unwrap();
                if matches!(return_type, FinalizedTypes::Generic(_, _)) {
                    let mut temp = vec![];
                    mem::swap(&mut temp, data.finalized_effects);
                    return Ok(Some(FinalizedEffectType::GenericVirtualCall(
                        i,
                        target,
                        AsyncDataGetter::new(data.code_verifier.syntax.clone(), found.clone()).await,
                        temp,
                        token,
                    )));
                }

                data.code_verifier.syntax.lock().unwrap().process_manager.handle().lock().unwrap().spawn(
                    target.name.clone(),
                    degeneric_header(
                        target.clone(),
                        found.clone(),
                        data.code_verifier.syntax.clone(),
                        data.code_verifier.process_manager.cloned(),
                        data.finalized_effects.clone(),
                        data.variables.clone(),
                        token.clone(),
                    ),
                );

                let output = AsyncDataGetter::new(data.code_verifier.syntax.clone(), target.clone()).await;
                let mut temp = vec![];
                mem::swap(&mut temp, data.finalized_effects);

                let output = CodelessFinalizedFunction::degeneric(
                    output,
                    Box::new(data.code_verifier.process_manager.clone()),
                    &temp,
                    &data.code_verifier.syntax,
                    data.variables,
                    None,
                )
                .await?;
                return Ok(Some(FinalizedEffectType::VirtualCall(i, output, temp)));
            }
            i += 1;
        }

        if !data.method.is_empty() {
            return Err(token.make_error("Unknown method!"));
        }
    }
    return Ok(None);
}

/// Tries to get an implementation matching the types passed in
async fn try_get_impl(data: &ImplCheckerData<'_>, span: &Span) -> Result<Option<FinalizedEffects>, ParsingError> {
    let result = ImplWaiter {
        syntax: data.code_verifier.syntax.clone(),
        return_type: data.finding_return_type.clone(),
        data: data.data.clone(),
        error: span.make_error("Nothing implements the given trait!"),
    }
    .await?;

    for temp in &result {
        if temp.name.split("::").last().unwrap() == data.method || data.method.is_empty() {
            let method = AsyncDataGetter::new(data.code_verifier.syntax.clone(), temp.clone()).await;

            let returning = match &data.returning {
                Some(inner) => Some((
                    Syntax::parse_type(
                        data.code_verifier.syntax.clone(),
                        span.make_error("Incorrect bounds!"),
                        data.code_verifier.resolver.boxed_clone(),
                        inner.clone(),
                        vec![],
                    )
                    .await?
                    .finalize(data.code_verifier.syntax.clone())
                    .await,
                    span.clone(),
                )),
                None => None,
            };

            match check_method(
                &data.code_verifier.process_manager,
                method.clone(),
                data.finalized_effects.clone(),
                &data.code_verifier.syntax,
                &data.variables,
                returning,
                span,
            )
            .await
            {
                Ok(found) => return Ok(Some(found)),
                Err(_error) => {}
            };
        }
    }
    return Ok(None);
}
