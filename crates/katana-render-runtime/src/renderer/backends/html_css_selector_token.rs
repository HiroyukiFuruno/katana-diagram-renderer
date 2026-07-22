use super::super::CssCombinator;

pub(super) enum SelectorToken {
    Compound(String),
    Combinator(CssCombinator),
}

#[derive(Default)]
struct SelectorTokenizer {
    tokens: Vec<SelectorToken>,
    current: String,
    bracket_depth: u8,
    quote: Option<char>,
    pending_descendant: bool,
}

pub(super) fn selector_tokens(selector: &str) -> Option<Vec<SelectorToken>> {
    if selector.is_empty() {
        return None;
    }
    let mut tokenizer = SelectorTokenizer::default();
    for character in selector.chars() {
        tokenizer.consume(character)?;
    }
    tokenizer.finish()
}

impl SelectorTokenizer {
    fn consume(&mut self, character: char) -> Option<()> {
        if self.consume_quoted(character) {
            return Some(());
        }
        match character {
            '+' if self.bracket_depth == 0 => return None,
            '~' if self.bracket_depth == 0 => return None,
            '\'' | '"' if self.bracket_depth > 0 => self.start_quote(character),
            '[' => self.open_attribute()?,
            ']' => self.close_attribute()?,
            '>' if self.bracket_depth == 0 => self.push_child()?,
            value if value.is_whitespace() && self.bracket_depth == 0 => self.push_whitespace(),
            value => self.push_value(value),
        }
        Some(())
    }

    fn consume_quoted(&mut self, character: char) -> bool {
        let Some(quote) = self.quote else {
            return false;
        };
        self.current.push(character);
        if quote == character {
            self.quote = None;
        }
        true
    }

    fn start_quote(&mut self, character: char) {
        self.quote = Some(character);
        self.current.push(character);
    }

    fn open_attribute(&mut self) -> Option<()> {
        self.bracket_depth = self.bracket_depth.checked_add(1)?;
        self.current.push('[');
        Some(())
    }

    fn close_attribute(&mut self) -> Option<()> {
        self.bracket_depth = self.bracket_depth.checked_sub(1)?;
        self.current.push(']');
        Some(())
    }

    fn push_child(&mut self) -> Option<()> {
        self.push_compound();
        if !matches!(self.tokens.last(), Some(SelectorToken::Compound(_))) {
            return None;
        }
        self.tokens
            .push(SelectorToken::Combinator(CssCombinator::Child));
        self.pending_descendant = false;
        Some(())
    }

    fn push_whitespace(&mut self) {
        self.push_compound();
        self.pending_descendant = matches!(self.tokens.last(), Some(SelectorToken::Compound(_)));
    }

    fn push_value(&mut self, value: char) {
        if self.pending_descendant {
            self.tokens
                .push(SelectorToken::Combinator(CssCombinator::Descendant));
            self.pending_descendant = false;
        }
        self.current.push(value);
    }

    fn push_compound(&mut self) {
        if !self.current.is_empty() {
            self.tokens
                .push(SelectorToken::Compound(std::mem::take(&mut self.current)));
        }
    }

    fn finish(mut self) -> Option<Vec<SelectorToken>> {
        if self.bracket_depth != 0 || self.quote.is_some() {
            return None;
        }
        self.push_compound();
        matches!(self.tokens.last(), Some(SelectorToken::Compound(_))).then_some(self.tokens)
    }
}
