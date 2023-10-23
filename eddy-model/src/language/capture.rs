#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Capture {
    Attribute,
    Comment,
    Constant,
    ConstantBuiltin,
    Constructor,
    Escape,
    Function,
    FunctionMacro,
    FunctionMethod,
    Keyword,
    Label,
    Operator,
    Property,
    PunctuationBracket,
    PunctuationDelimiter,
    String,
    Type,
    TypeBuiltin,
    VariableBuiltin,
    VariableParameter,
}

impl Capture {
    pub fn from_name(name: &str) -> Option<Capture> {
        match name {
            "attribute" => Some(Capture::Attribute),
            "comment" => Some(Capture::Comment),
            "constant" => Some(Capture::Constant),
            "constant.builtin" => Some(Capture::ConstantBuiltin),
            "constructor" => Some(Capture::Constructor),
            "escape" => Some(Capture::Escape),
            "function" => Some(Capture::Function),
            "function.macro" => Some(Capture::FunctionMacro),
            "function.method" => Some(Capture::FunctionMethod),
            "keyword" => Some(Capture::Keyword),
            "label" => Some(Capture::Label),
            "operator" => Some(Capture::Operator),
            "property" => Some(Capture::Property),
            "punctuation.bracket" => Some(Capture::PunctuationBracket),
            "punctuation.delimiter" => Some(Capture::PunctuationDelimiter),
            "string" => Some(Capture::String),
            "type" => Some(Capture::Type),
            "type.builtin" => Some(Capture::TypeBuiltin),
            "variable.builtin" => Some(Capture::VariableBuiltin),
            "variable.parameter" => Some(Capture::VariableParameter),
            _ => None,
        }
    }
}
