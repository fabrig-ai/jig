//! Whole specification values and explicit document loss (client/domain/contract.md, 5.2–5.3).
use super::{DocumentText, Text};
use skein_lib::List;

/// A complete replacement, with numbered parameters and historical input identities.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Specification {
    pub words: Text,
    pub parameters: List<Parameter>,
    pub inputs: List<u64>,
    pub asks: bool,
}

/// A supplied specification and its owner's explicit loss marker.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpecificationDocument {
    pub specification: DocumentSpecification,
    pub cut: bool,
}

/// A named specification value; text is UTF-8 and resources retain their connector identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Parameter {
    Number { name: u32, value: u64 },
    Bytes { name: u32, value: Text },
    Resource { name: u32, connector: u16, resource: u64 },
}

/// The independently marked text and lists supplied in a specification document.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentSpecification {
    pub words: DocumentText,
    pub parameters: List<ParameterDocument>,
    pub inputs: List<u64>,
    pub asks: bool,
}
/// A document parameter; only a byte value has a text loss fact.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ParameterDocument {
    Number { name: u32, value: u64 },
    Bytes { name: u32, value: DocumentText },
    Resource { name: u32, connector: u16, resource: u64 },
}
impl SpecificationDocument {
    /// A caller supplies a complete specification, with no omitted values.
    #[must_use]
    pub fn complete(specification: Specification) -> Self {
        let mut parameters = List::with_capacity(specification.parameters.capacity());
        for value in &specification.parameters {
            parameters
                .push(match value {
                    Parameter::Number { name, value } => ParameterDocument::Number { name: *name, value: *value },
                    Parameter::Bytes { name, value } => {
                        ParameterDocument::Bytes { name: *name, value: DocumentText::complete(value.clone()) }
                    }
                    Parameter::Resource { name, connector, resource } => {
                        ParameterDocument::Resource { name: *name, connector: *connector, resource: *resource }
                    }
                })
                .expect("same supplied parameter capacity");
        }
        Self {
            specification: DocumentSpecification {
                asks: specification.asks,
                words: DocumentText::complete(specification.words),
                parameters,
                inputs: specification.inputs,
            },
            cut: false,
        }
    }
    /// Whether any independently supplied text or list is incomplete.
    #[must_use]
    pub fn has_loss(&self) -> bool {
        if self.cut || self.specification.words.cut {
            return true;
        }
        for value in &self.specification.parameters {
            match value {
                ParameterDocument::Bytes { value, .. } if value.cut => return true,
                ParameterDocument::Bytes { .. }
                | ParameterDocument::Number { .. }
                | ParameterDocument::Resource { .. } => {}
            }
        }
        false
    }
    /// A whole replacement is available only when every supplied component is complete.
    #[must_use]
    pub fn whole(&self) -> Option<Specification> {
        if self.has_loss() {
            return None;
        }
        let mut parameters = List::with_capacity(self.specification.parameters.capacity());
        for value in &self.specification.parameters {
            parameters
                .push(match value {
                    ParameterDocument::Number { name, value } => Parameter::Number { name: *name, value: *value },
                    ParameterDocument::Bytes { name, value } => Parameter::Bytes { name: *name, value: value.whole()? },
                    ParameterDocument::Resource { name, connector, resource } => {
                        Parameter::Resource { name: *name, connector: *connector, resource: *resource }
                    }
                })
                .expect("same supplied parameter capacity");
        }
        Some(Specification {
            asks: self.specification.asks,
            words: self.specification.words.whole()?,
            parameters,
            inputs: self.specification.inputs.clone(),
        })
    }
}
