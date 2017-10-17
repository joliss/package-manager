#![allow(dead_code,unused_variables,unused_assignments)]

use pest;
use pest::Parser;

// Ensure this file recompiles when the grammar is modified.
const _GRAMMAR: &'static str = include_str!("grammar.pest");

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct ManifestParser;

pub type Error = pest::Error<Rule, pest::inputs::StringInput>;

pub type Pair = pest::iterators::Pair<Rule, pest::inputs::StringInput>;
pub type Pairs = pest::iterators::Pairs<Rule, pest::inputs::StringInput>;


pub fn parse_manifest(manifest_source: String)
    -> Result<Pair, Error>
{
    let parser_input = ::std::rc::Rc::new(::pest::inputs::StringInput::new(manifest_source));
    let mut pairs = ManifestParser::parse(Rule::manifest_eof, parser_input)?;
    let manifest_eof_pair = pairs.next().expect("returns exactly one pair");
    let manifest_pair = find_rule(manifest_eof_pair, Rule::manifest);
    Ok(manifest_pair)
}

// Check that there are no unexpected or duplicate fields.
pub fn check_block_fields(block_pair: Pair, fields: &'static [&'static str])
    -> Result<(), Error>
{
    let mut seen = vec![false; fields.len()];
    'pair_loop: for (symbol_pair, _arguments_pair) in get_fields(block_pair) {
        let symbol = symbol_pair.as_str();
        for i in 0..fields.len() {
            if symbol == fields[i] {
                if seen[i] {
                    return Err(pest::Error::CustomErrorSpan {
                        message: "Duplicate field".to_string(),
                        span: symbol_pair.clone().into_span(),
                    })
                } else {
                    seen[i] = true;
                    continue 'pair_loop;
                }
            }
        }
        return Err(pest::Error::CustomErrorSpan {
            message: "Unexpected field".to_string(),
            span: symbol_pair.clone().into_span(),
        });
    }
    Ok(())
}

// Return an arguments pair or an error if the field is missing.
pub fn get_field(
    block_pair: Pair,
    field_name: &'static str)
    -> Result<Pair, Error>
{
    get_optional_field(block_pair.clone(), field_name)
        .ok_or_else(||
            pest::Error::CustomErrorSpan {
                message: format!("Missing field: {}", field_name),
                // We probably want to report this on the line following the
                // opening brace instead.
                span: block_pair.into_span(),
            }
        )
}

pub fn get_optional_field(
    block_pair: Pair, field_name: &'static str)
    -> Option<Pair>
{
    for (symbol_pair, arguments_pair) in get_fields(block_pair) {
        if symbol_pair.as_str() == field_name {
            return Some(arguments_pair);
        }
    }
    None
}

pub struct Arguments {
    pub positional_arguments_pair: Pair,
    pub positional_arguments: Vec<Pair>,
    // pub options_pair: Pair,
    // pub options: ...
    pub block: Option<Pair>
}

impl Arguments {
    /// Get a single positional argument.
    pub fn get_single(arguments_pair: Pair)
        -> Result<Pair, Error>
    {
        let arguments = Arguments::from_pair(arguments_pair, 1, 1, Some(false))?;
        Ok(arguments.positional_arguments[0].clone())
    }

    /// Create an `Arguments` instance from `arguments_pair`, validating that
    /// the right number of arguments is supplied. If `expect_block` is `None`,
    /// the block is optional.
    pub fn from_pair(
        arguments_pair: Pair,
        min_positional_arguments: usize, max_positional_arguments: usize,
        expect_block: Option<bool>
        ) -> Result<Self, Error>
    {
        let positional_arguments_pair = find_rule(arguments_pair.clone(), Rule::positional_arguments);
        let positional_arguments = children(positional_arguments_pair.clone(), Rule::positional_argument);

        if min_positional_arguments == max_positional_arguments {
            if positional_arguments.len() != min_positional_arguments {
                return Err(Error::from(pest::Error::CustomErrorSpan {
                    message: format!("Expected {} argument(s), found {}",
                        min_positional_arguments, positional_arguments.len()),
                    span: positional_arguments_pair.into_span()
                }));
            }
        } else {
            if !(positional_arguments.len() >= min_positional_arguments) {
                return Err(Error::from(pest::Error::CustomErrorSpan {
                    message: format!("Expected at least {} argument(s), found {}",
                        min_positional_arguments, positional_arguments.len()),
                    span: positional_arguments_pair.into_span()
                }));
            }
            if !(positional_arguments.len() <= max_positional_arguments) {
                return Err(Error::from(pest::Error::CustomErrorSpan {
                    message: format!("Expected at most {} argument(s), found {}",
                        min_positional_arguments, positional_arguments.len()),
                    span: positional_arguments_pair.into_span(),
                }));
            }
        }

        let maybe_block = find_optional_rule(arguments_pair.clone(), Rule::block);
        match expect_block {
            None => { }
            Some(true) => {
                if maybe_block.is_none() {
                    return Err(Error::from(pest::Error::CustomErrorPos {
                        message: "Expected `{`".to_string(),
                        pos: arguments_pair.into_span().end_pos(),
                    }));
                }
            },
            Some(false) => {
                if let Some(block) = maybe_block {
                    return Err(Error::from(pest::Error::CustomErrorSpan {
                        message: "Unexpected block".to_string(),
                        span: block.into_span(),
                    }));
                }
            },
        }

        Ok(Arguments {
            positional_arguments_pair,
            positional_arguments,
            block: maybe_block,
        })
    }
}

pub fn get_optional_block_field(
    block_pair: Pair, field_name: &'static str)
    -> Result<Vec<(Pair, Pair)>, Error>
{
    get_optional_field(block_pair, field_name)
        .map_or(Ok(vec![]), |arguments_pair| {
            let arguments = Arguments::from_pair(arguments_pair, 0, 0, Some(true))?;
            Ok(get_fields(arguments.block.expect("validated block presence")))
        })
}

pub fn get_optional_list_field(
    block_pair: Pair, field_name: &'static str)
    -> Result<Vec<Pair>, Error>
{
    get_optional_field(block_pair, field_name)
        .map_or(Ok(vec![]), |arguments_pair| {
            let argument_pair = Arguments::get_single(arguments_pair)?;
            Ok(get_list(argument_pair)?)
        })
}

pub fn get_optional_string_field(block_pair: Pair, field_name: &'static str)
    -> Result<Option<String>, Error>
{
    get_optional_field(block_pair, field_name)
        .map_or(Ok(None), |arguments_pair| {
            let argument_pair = Arguments::get_single(arguments_pair)?;
            Ok(Some(get_string(argument_pair)?))
        })
}

pub fn get_fields(block_pair: Pair) -> Vec<(Pair, Pair)> {
    let fields_pair = find_optional_rule(block_pair.clone(), Rule::fields_newline_terminated)
        .unwrap_or_else(|| find_rule(block_pair.clone(), Rule::fields_not_newline_terminated));
    children(fields_pair, Rule::field).into_iter().map(|field|
        (
            find_rule(field.clone(), Rule::symbol),
            find_rule(field.clone(), Rule::arguments)
        )
    ).collect()
}

pub fn get_string(pair: Pair) -> Result<String, Error> {
    for string_pair in children(pair.clone(), Rule::string) {
        let string_contents = parse_string(string_pair.clone())?;
        return Ok(string_contents);
    }
    Err(pest::Error::CustomErrorSpan {
        message: "Expected string".to_string(),
        span: pair.into_span(),
    })
}

pub fn get_list(pair: Pair) -> Result<Vec<Pair>, Error> {
    for list_pair in children(pair.clone(), Rule::list) {
        let list_item_pairs = children(list_pair.clone(), Rule::list_item);
        return Ok(list_item_pairs);
    }
    Err(pest::Error::CustomErrorSpan {
        message: "Expected list".to_string(),
        span: pair.into_span(),
    })
}

pub fn get_version_constraint_component(pair: Pair) -> Result<String, Error> {
    for vcc_pair in children(pair.clone(), Rule::version_constraint_component) {
        let version_constraint_component = vcc_pair.as_str().to_string();
        return Ok(version_constraint_component);
    }
    Err(pest::Error::CustomErrorSpan {
        message: "Expected version range".to_string(),
        span: pair.into_span(),
    })
}

pub fn parse_string(string_pair: Pair) -> Result<String, Error> {
    let mut s = "".to_string();
    for pair in string_pair.into_inner() {
        let c = match pair.as_rule() {
            Rule::literal_character => pair.as_str().chars().next().unwrap(),
            Rule::escaped_quote => '"',
            Rule::escaped_backslash => '\\',
            Rule::escaped_newline => '\n',
            Rule::escaped_tab => '\t',
            Rule::escaped_unicode => {
                let mut hex = "".to_string();
                for hex_pair in children(pair.clone(), Rule::hex) {
                    hex.push_str(hex_pair.as_str());
                }
                let cp = u32::from_str_radix(&hex, 16).expect("parser should not return invalid hex strings");
                match ::std::char::from_u32(cp) {
                    None => {
                        return Err(pest::Error::CustomErrorSpan {
                            message: "Invalid unicode scalar".to_string(),
                            span: pair.into_span(),
                        });
                    },
                    Some(c) => c
                }
            }
            _ => unreachable!("unexpected string character rule; maybe we didn't get a string"),
        };
        s.push(c);
    }
    Ok(s)
}

pub fn children(pair: Pair, rule: Rule) -> Vec<Pair> {
    children_of_pairs(pair.into_inner(), rule)
}

pub fn children_of_pairs(pairs: Pairs, rule: Rule) -> Vec<Pair> {
    pairs.filter(|pair| pair.as_rule() == rule).collect()
}

pub fn find_rule(pair: Pair, rule: Rule) -> Pair {
    find_optional_rule(pair, rule)
        .unwrap_or_else(||
            // Closure makes error message formatting lazy.
            panic!("No child matches rule {:?}", rule)
        )

}

pub fn find_optional_rule(pair: Pair, rule: Rule) -> Option<Pair> {
    pair.into_inner().find(|child_pair| child_pair.as_rule() == rule)
}
