use super::{CssDeclaration, CssRule, declarations_for};
use crate::renderer::backends::html_css_selector::CssSelector;
use cssparser::{
    AtRuleParser, BasicParseErrorKind, CowRcStr, DeclarationParser, ParseError, Parser,
    ParserInput, ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser,
    StyleSheetParser, Token,
};

#[path = "html_css_parser_value.rs"]
mod value;
use value::{normalized_property_name, parse_declaration_value};

pub(super) fn rules(source: &str) -> Vec<CssRule> {
    let mut input = ParserInput::new(source);
    let mut input = Parser::new(&mut input);
    parse_stylesheet(&mut input, Vec::new())
}

pub(super) fn declarations(source: &str) -> Vec<CssDeclaration> {
    let mut input = ParserInput::new(source);
    let mut input = Parser::new(&mut input);
    parse_declaration_body(&mut input)
}

fn parse_stylesheet<'i, 't>(input: &mut Parser<'i, 't>, media: Vec<String>) -> Vec<CssRule> {
    let mut parser = StylesheetRuleParser { media };
    StyleSheetParser::new(input, &mut parser)
        .filter_map(Result::ok)
        .flatten()
        .collect()
}

struct StylesheetRuleParser {
    media: Vec<String>,
}

impl<'i> QualifiedRuleParser<'i> for StylesheetRuleParser {
    type Prelude = Vec<CssSelector>;
    type QualifiedRule = Vec<CssRule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        input.parse_comma_separated(|selector| {
            let source = consume_source(selector);
            CssSelector::parse(source.trim()).ok_or_else(|| selector.new_custom_error(()))
        })
    }

    fn parse_block<'t>(
        &mut self,
        selectors: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let declarations = parse_declaration_body(input);
        if selectors.is_empty() || declarations.is_empty() {
            return Ok(Vec::new());
        }
        Ok(vec![CssRule {
            selectors,
            declarations,
            media: self.media.clone(),
        }])
    }
}

impl<'i> AtRuleParser<'i> for StylesheetRuleParser {
    type Prelude = String;
    type AtRule = Vec<CssRule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        if !name.eq_ignore_ascii_case("media") {
            return Err(input.new_error(BasicParseErrorKind::AtRuleInvalid(name)));
        }
        Ok(consume_source(input).trim().to_string())
    }

    fn parse_block<'t>(
        &mut self,
        query: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::AtRule, ParseError<'i, Self::Error>> {
        let mut media = self.media.clone();
        media.push(query);
        Ok(parse_stylesheet(input, media))
    }
}

fn parse_declaration_body<'i, 't>(input: &mut Parser<'i, 't>) -> Vec<CssDeclaration> {
    let mut parser = CssDeclarationParser;
    RuleBodyParser::new(input, &mut parser)
        .filter_map(Result::ok)
        .flatten()
        .collect()
}

struct CssDeclarationParser;

impl<'i> DeclarationParser<'i> for CssDeclarationParser {
    type Declaration = Vec<CssDeclaration>;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let (value, important) = parse_declaration_value(input)?;
        let name = normalized_property_name(name);
        Ok(declarations_for(name, value, important))
    }
}

impl<'i> AtRuleParser<'i> for CssDeclarationParser {
    type Prelude = ();
    type AtRule = Vec<CssDeclaration>;
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for CssDeclarationParser {
    type Prelude = ();
    type QualifiedRule = Vec<CssDeclaration>;
    type Error = ();
}

impl RuleBodyItemParser<'_, Vec<CssDeclaration>, ()> for CssDeclarationParser {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn consume_source<'i>(input: &mut Parser<'i, '_>) -> &'i str {
    let start = input.position();
    let _ = consume_nested(input);
    input.slice_from(start)
}

fn consume_nested<'i, 't>(input: &mut Parser<'i, 't>) -> Result<(), ParseError<'i, ()>> {
    while !input.is_exhausted() {
        let token = input.next_including_whitespace_and_comments()?.clone();
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            input.parse_nested_block(consume_nested)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{consume_source, declarations, rules};

    #[test]
    fn parser_skips_empty_rules_and_returns_no_declarations() {
        let stylesheet = "div {} .box { color: red; }\n";
        let parsed = rules(stylesheet);

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].declarations.len(), 1);
        assert_eq!(parsed[0].declarations[0].name, "color");
    }

    #[test]
    fn parser_skips_unsupported_at_rules() {
        let parsed = rules("@font-face { font-family: OpenSans; } .card { width: 10px; }");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].declarations[0].name, "width");
    }

    #[test]
    fn parser_collects_declarations() {
        let declarations = declarations("margin: 4px; color: red;");

        let names = declarations
            .iter()
            .map(|declaration| declaration.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "margin-top",
                "margin-right",
                "margin-bottom",
                "margin-left",
                "color"
            ]
        );
        assert!(
            declarations
                .iter()
                .all(|declaration| !declaration.important)
        );
    }

    #[test]
    fn parser_nested_tokens_are_consumed_in_source_slice() {
        let mut parser_input = cssparser::ParserInput::new("func(a(b(c)));");
        let mut parser = cssparser::Parser::new(&mut parser_input);
        let source = consume_source(&mut parser);

        assert_eq!(source, "func(a(b(c)));");
    }
}
