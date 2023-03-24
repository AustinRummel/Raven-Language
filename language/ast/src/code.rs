use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::mem;
use crate::{assign_with_priority, DisplayIndented, to_modifiers};
use crate::blocks::IfStatement;
use crate::function::{Arguments, CodeBody, display_joined};
use crate::type_resolver::FinalizedTypeResolver;
use crate::types::ResolvableTypes;

#[derive(Clone)]
pub struct Expression {
    pub expression_type: ExpressionType,
    pub effect: Effects,
}

#[derive(Clone, Copy)]
pub enum ExpressionType {
    Break,
    Return,
    Line,
}

#[derive(Clone)]
pub struct Field {
    pub name: String,
    pub field_type: ResolvableTypes,
}

#[derive(Clone)]
pub struct MemberField {
    pub modifiers: u8,
    pub field: Field,
}

impl MemberField {
    pub fn new(modifiers: u8, field: Field) -> Self {
        return Self {
            modifiers,
            field,
        };
    }
}

impl DisplayIndented for MemberField {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}{} {};", indent, display_joined(&to_modifiers(self.modifiers)), self.field);
    }
}

impl Expression {
    pub fn new(expression_type: ExpressionType, effect: Effects) -> Self {
        return Self {
            expression_type,
            effect,
        };
    }

    pub fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.effect.finalize(type_resolver);
    }

    pub fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        self.effect.as_mut().set_generics(replacing);
    }

    pub fn is_return(&self) -> bool {
        return if let ExpressionType::Return = self.expression_type {
            true
        } else {
            self.effect.unwrap().is_return()
        };
    }
}

impl Field {
    pub fn new(name: String, field_type: ResolvableTypes) -> Self {
        return Self {
            name,
            field_type,
        };
    }

    pub fn set_generics(&self, replacing: &HashMap<String, ResolvableTypes>) -> Self {
        let mut field = self.clone();
        field.field_type.set_generic(replacing);
        return field;
    }

    pub fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        type_resolver.finalize(&mut self.field_type);
    }
}

impl DisplayIndented for Expression {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", indent)?;
        match self.expression_type {
            ExpressionType::Return => write!(f, "return")?,
            ExpressionType::Break => write!(f, "break")?,
            _ => {}
        }
        if let Effects::NOP() = self.effect {
            return write!(f, ";\n");
        } else if let ExpressionType::Line = self.expression_type {
            //Only add a space for returns
        } else {
            write!(f, " ")?;
        }
        self.effect.format(indent, f)?;
        if self.effect.unwrap().has_return() {
            return write!(f, ";\n");
        }
        return write!(f, "\n");
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}: {}", self.name, self.field_type);
    }
}

pub trait Effect: DisplayIndented {
    fn is_return(&self) -> bool;

    fn has_return(&self) -> bool;

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver);

    fn return_type(&self) -> Option<ResolvableTypes>;

    fn get_location(&self) -> (u32, u32);

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>);
}

#[derive(Clone)]
pub enum Effects {
    NOP(),
    Wrapped(Box<Effects>),
    CodeBody(Box<CodeBody>),
    IfStatement(Box<IfStatement>),
    MethodCall(Box<MethodCall>),
    VariableLoad(Box<VariableLoad>),
    FieldLoad(Box<FieldLoad>),
    CreateStruct(Box<CreateStruct>),
    FloatEffect(Box<NumberEffect<f64>>),
    IntegerEffect(Box<NumberEffect<i64>>),
    AssignVariable(Box<AssignVariable>),
    OperatorEffect(Box<OperatorEffect>),
}

impl Effects {
    pub fn unwrap(&self) -> &dyn Effect {
        return match self {
            Effects::NOP() => panic!("Tried to unwrap a NOP!"),
            Effects::Wrapped(effect) => effect.unwrap(),
            Effects::CodeBody(effect) => effect.as_ref(),
            Effects::IfStatement(effect) => effect.as_ref(),
            Effects::MethodCall(effect) => effect.as_ref(),
            Effects::VariableLoad(effect) => effect.as_ref(),
            Effects::FieldLoad(effect) => effect.as_ref(),
            Effects::CreateStruct(effect) => effect.as_ref(),
            Effects::FloatEffect(effect) => effect.as_ref(),
            Effects::IntegerEffect(effect) => effect.as_ref(),
            Effects::AssignVariable(effect) => effect.as_ref(),
            Effects::OperatorEffect(effect) => effect.as_ref()
        };
    }

    pub fn as_mut(&mut self) -> &mut dyn Effect {
        return match self {
            Effects::NOP() => panic!("Tried to unwrap a NOP!"),
            Effects::Wrapped(effect) => Effects::as_mut(effect),
            Effects::CodeBody(effect) => effect.as_mut(),
            Effects::IfStatement(effect) => effect.as_mut(),
            Effects::MethodCall(effect) => effect.as_mut(),
            Effects::VariableLoad(effect) => effect.as_mut(),
            Effects::FieldLoad(effect) => effect.as_mut(),
            Effects::CreateStruct(effect) => effect.as_mut(),
            Effects::FloatEffect(effect) => effect.as_mut(),
            Effects::IntegerEffect(effect) => effect.as_mut(),
            Effects::AssignVariable(effect) => effect.as_mut(),
            Effects::OperatorEffect(effect) => effect.as_mut()
        };
    }

    pub fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.as_mut().finalize(type_resolver);
    }
}

impl Display for Effects {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl DisplayIndented for Effects {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return match self {
            Effects::Wrapped(effect) => {
                write!(f, "(")?;
                effect.format(indent, f)?;
                write!(f, ")")
            }
            Effects::NOP() => write!(f, "{{}}"),
            _ => self.unwrap().format(indent, f)
        };
    }
}

#[derive(Clone)]
pub struct FieldLoad {
    pub calling: Effects,
    pub name: String,
    loc: (u32, u32),
}

impl FieldLoad {
    pub fn new(calling: Effects, name: String, loc: (u32, u32)) -> Self {
        return Self {
            calling,
            name,
            loc,
        };
    }
}

impl Effect for FieldLoad {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.calling.finalize(type_resolver);
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        for field in self.calling.unwrap().return_type().as_ref().unwrap().unwrap().get_fields() {
            if field.field.name == self.name {
                return Some(field.field.field_type.clone());
            }
        }
        panic!("Failed to find return type!")
    }

    fn get_location(&self) -> (u32, u32) {
        return self.loc;
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        self.calling.as_mut().set_generics(replacing);
    }
}

impl DisplayIndented for FieldLoad {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.calling.format(parsing, f)?;
        return write!(f, ".{}", self.name);
    }
}

#[derive(Clone)]
pub struct MethodCall {
    pub calling: Option<Effects>,
    pub method: String,
    pub method_return: Option<ResolvableTypes>,
    pub arguments: Arguments,
    location: (u32, u32),
}

impl MethodCall {
    pub fn new(calling: Option<Effects>, method: String, arguments: Arguments, location: (u32, u32)) -> Self {
        return Self {
            calling,
            method,
            method_return: None,
            arguments,
            location,
        };
    }
}

impl Effect for MethodCall {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.arguments.finalize(type_resolver);

        let mut method = self.method.clone();

        if self.calling.is_some() {
            self.calling.as_mut().unwrap().finalize(type_resolver);
            let returned = self.calling.as_mut().unwrap().unwrap().return_type();
            println!("Calling {}", returned.as_ref().map(|types| types.to_string()).unwrap_or("None".to_string()));
            for func in &returned.as_ref().unwrap().unwrap().structure.functions {
                println!("Testing {}", func);
                if func.split("::").last().unwrap() == method {
                    method = func.clone();
                    break
                }
            }
        }

        self.method_return = match type_resolver.get_function(&method) {
            Some(func) => if func.generics.is_empty() {
                func.return_type.clone()
            } else {
                let func = type_resolver.solidify_generics(&self.method, func.extract_generics(
                    &self.arguments.arguments.iter().map(|arg| arg.unwrap().return_type().unwrap()).collect()
                ));
                self.method = func.name.clone();
                func.return_type.clone()
            },
            None => {
                panic!("No method named {}!", self.method)
            }
        };
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return self.method_return.clone();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        if let Some(found) = &mut self.calling {
            found.as_mut().set_generics(replacing);
        }

        if let Some(returned) = &self.method_return {
            self.method_return = Some(returned.clone());
            self.method_return.as_mut().unwrap().set_generic(replacing);
        }
    }
}

impl DisplayIndented for MethodCall {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(value) = &self.calling {
            value.format(indent, f)?;
            write!(f, ".")?;
        }
        return write!(f, "{}{}", self.method, self.arguments);
    }
}

#[derive(Clone)]
pub struct CreateStruct {
    pub structure: ResolvableTypes,
    pub parsed_effects: Option<Vec<(usize, Effects)>>,
    pub effects: Option<Vec<(String, Effects)>>,
    location: (u32, u32),
}

impl CreateStruct {
    pub fn new(structure: ResolvableTypes, effects: Vec<(String, Effects)>, location: (u32, u32)) -> Self {
        return Self {
            structure,
            parsed_effects: None,
            effects: Some(effects),
            location,
        };
    }
}

impl Effect for CreateStruct {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.structure.finalize(type_resolver);
        let structure = &self.structure.unwrap();

        let mut output = Vec::new();

        let mut temp = None;
        mem::swap(&mut temp, &mut self.effects);

        for (name, mut effect) in temp.unwrap() {
            effect.finalize(type_resolver);
            let fields = structure.get_fields();
            for i in 0..fields.len() {
                let field = fields.get(i).unwrap();
                if field.field.name == name {
                    output.push((i, effect));
                    break;
                }
            }
        }

        self.parsed_effects = Some(output);
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return Some(self.structure.clone());
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        self.structure.set_generic(replacing);
        if let Some(effects) = &mut self.parsed_effects {
            for (_name, effect) in effects {
                effect.as_mut().set_generics(replacing);
            }
        }
    }
}

impl DisplayIndented for CreateStruct {
    fn format(&self, parsing: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {{\n", self.structure)?;
        let deeper_indent = parsing.to_string() + "    ";
        let deeper_indent = deeper_indent.as_str();
        let deepest_indent = deeper_indent.to_string() + "    ";
        let deepest_indent = deepest_indent.as_str();
        match self.effects.as_ref() {
            Some(effects) => {
                for (name, effect) in effects {
                    write!(f, "{}{}: ", deeper_indent, name)?;
                    DisplayIndented::format(effect, deepest_indent, f)?;
                    write!(f, "\n")?;
                }
            }
            None => {
                for (loc, effect) in self.parsed_effects.as_ref().unwrap() {
                    write!(f, "{}{}: ", deeper_indent,
                           self.structure.unwrap().get_fields().get(*loc).unwrap().field.name)?;
                    DisplayIndented::format(effect, deepest_indent, f)?;
                    write!(f, "\n")?;
                }
            }
        }
        return write!(f, "{}}}", parsing);
    }
}

#[derive(Clone)]
pub struct VariableLoad {
    pub name: String,
    pub types: Option<ResolvableTypes>,
    location: (u32, u32),
}

impl VariableLoad {
    pub fn new(name: String, location: (u32, u32)) -> Self {
        return Self {
            name,
            types: None,
            location,
        };
    }
}

impl Effect for VariableLoad {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.types = Some(type_resolver.get_variable(&self.name).expect(format!("Unknown variable {}", self.name).as_str()).clone());
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return self.types.clone();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        if let Some(types) = &self.types {
            let mut types = types.clone();
            types.set_generic(replacing);
            self.types = Some(types);
        }
    }
}

impl DisplayIndented for VariableLoad {
    fn format(&self, _indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.name);
    }
}

#[derive(Clone)]
pub struct NumberEffect<T> where T: Display + Typed {
    pub return_type: ResolvableTypes,
    pub number: T,
}

impl<T> NumberEffect<T> where T: Display + Typed {
    pub fn new(number: T) -> Self {
        return Self {
            return_type: T::get_type(),
            number,
        };
    }
}

pub trait Typed {
    fn get_type() -> ResolvableTypes;
}

impl Typed for f64 {
    fn get_type() -> ResolvableTypes {
        return ResolvableTypes::Resolving("f64".to_string());
    }
}

impl Typed for i64 {
    fn get_type() -> ResolvableTypes {
        return ResolvableTypes::Resolving("i64".to_string());
    }
}

impl<T> Effect for NumberEffect<T> where T: Display + Typed {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.return_type.finalize(type_resolver);
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return Some(self.return_type.clone());
    }

    fn get_location(&self) -> (u32, u32) {
        panic!("Unexpected get location!");
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        self.return_type.set_generic(replacing);
    }
}

impl<T> DisplayIndented for NumberEffect<T> where T: Display + Typed {
    fn format(&self, _indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        return write!(f, "{}", self.number);
    }
}

#[derive(Clone)]
pub struct AssignVariable {
    pub variable: String,
    pub effect: Effects,
    location: (u32, u32),
}

impl AssignVariable {
    pub fn new(variable: String, effect: Effects, location: (u32, u32)) -> Self {
        return Self {
            variable,
            effect,
            location,
        };
    }
}

impl Effect for AssignVariable {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        self.effect.finalize(type_resolver);
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return self.effect.unwrap().return_type();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        self.effect.as_mut().set_generics(replacing);
    }
}

impl DisplayIndented for AssignVariable {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "let {} = ", self.variable)?;
        return self.effect.format(indent, f);
    }
}

#[derive(Clone)]
pub struct OperatorEffect {
    pub operator: String,
    pub function: Option<String>,
    pub effects: Vec<Effects>,
    pub priority: i8,
    pub parse_left: bool,
    return_type: Option<ResolvableTypes>,
    location: (u32, u32),
}

impl OperatorEffect {
    pub fn new(operator: String, effects: Vec<Effects>, location: (u32, u32)) -> Self {
        return Self {
            operator,
            function: None,
            effects,
            priority: -100,
            parse_left: false,
            return_type: None,
            location,
        };
    }
}

impl Display for OperatorEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        return self.format("", f);
    }
}

impl Effect for OperatorEffect {
    fn is_return(&self) -> bool {
        return false;
    }

    fn has_return(&self) -> bool {
        return true;
    }

    fn finalize(&mut self, type_resolver: &mut dyn FinalizedTypeResolver) {
        for effect in &mut self.effects {
            effect.finalize(type_resolver);
        }

        let function = type_resolver.get_operator(&self.effects, self.operator.clone()).unwrap();
        self.function = Some(function.name.clone());
        self.return_type = function.return_type.clone();

        self.priority = function.attributes.get("priority")
            .map_or(0, |attrib| attrib.value.parse().expect("Expected numerical priority!"));
        self.parse_left = function.attributes.get("parse_left")
            .map_or(true, |attrib| attrib.value.parse().expect("Expected boolean parse_left!"));

        let mut temp = OperatorEffect::new(String::new(), Vec::new(), (0, 0));
        mem::swap(&mut temp, self);
        *self = assign_with_priority(Box::new(temp));
    }

    fn return_type(&self) -> Option<ResolvableTypes> {
        return self.return_type.clone();
    }

    fn get_location(&self) -> (u32, u32) {
        return self.location;
    }

    fn set_generics(&mut self, replacing: &HashMap<String, ResolvableTypes>) {
        if let Some(returning) = &self.return_type {
            let mut returning = returning.clone();
            returning.set_generic(replacing);
            self.return_type = Some(returning);
        }

        for effects in &mut self.effects {
            effects.as_mut().set_generics(replacing);
        }
    }
}

impl DisplayIndented for OperatorEffect {
    fn format(&self, indent: &str, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut skipping = false;
        let mut effects = self.effects.iter();
        //Get the operation itself
        for char in self.operator.split("::").last().unwrap().chars() {
            //Replace placeholders
            if skipping {
                skipping = false;
                effects.next().unwrap().format(indent, f)?;
            } else if char == '{' {
                skipping = true;
            } else {
                write!(f, "{}", char)?;
            }
        }

        return Ok(());
    }
}