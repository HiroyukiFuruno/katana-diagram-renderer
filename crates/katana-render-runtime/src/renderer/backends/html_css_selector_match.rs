use super::{
    CssAncestor, CssAttributeSelector, CssCombinator, CssCompoundSelector, CssNthExpression,
    CssPseudoElement, CssSelector, HtmlAttributes,
};

impl CssSelector {
    pub(in crate::renderer::backends) fn matches(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
    ) -> bool {
        self.matches_at(tag, attributes, ancestors, 1)
    }

    pub(in crate::renderer::backends) fn matches_at(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
    ) -> bool {
        self.matches_at_state(tag, attributes, ancestors, sibling_index, false)
    }

    pub(in crate::renderer::backends) fn matches_at_state(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
    ) -> bool {
        self.matches_at_pseudo_state(tag, attributes, ancestors, sibling_index, hovered, None)
    }

    pub(in crate::renderer::backends) fn matches_at_pseudo_state(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
        pseudo_element: Option<CssPseudoElement>,
    ) -> bool {
        if self.pseudo_element() != pseudo_element {
            return false;
        }
        self.matches_from(
            self.compounds.len() - 1,
            tag,
            attributes,
            ancestors,
            sibling_index,
            hovered,
        )
    }

    fn matches_from(
        &self,
        index: usize,
        tag: &str,
        attributes: &HtmlAttributes,
        ancestors: &[CssAncestor],
        sibling_index: usize,
        hovered: bool,
    ) -> bool {
        if !self.compounds[index].matches(tag, attributes, sibling_index, hovered) {
            return false;
        }
        if index == 0 {
            return true;
        }
        match self.combinators[index - 1] {
            CssCombinator::Child => self.matches_parent(index, ancestors),
            CssCombinator::Descendant => self.matches_ancestor(index, ancestors),
        }
    }

    fn matches_parent(&self, index: usize, ancestors: &[CssAncestor]) -> bool {
        ancestors.last().is_some_and(|parent| {
            self.matches_from(
                index - 1,
                &parent.tag,
                &parent.attributes,
                &ancestors[..ancestors.len() - 1],
                parent.sibling_index,
                parent.hovered,
            )
        })
    }

    fn matches_ancestor(&self, index: usize, ancestors: &[CssAncestor]) -> bool {
        ancestors.iter().enumerate().rev().any(|(position, item)| {
            self.matches_from(
                index - 1,
                &item.tag,
                &item.attributes,
                &ancestors[..position],
                item.sibling_index,
                item.hovered,
            )
        })
    }
}

impl CssCompoundSelector {
    fn matches(
        &self,
        tag: &str,
        attributes: &HtmlAttributes,
        sibling_index: usize,
        hovered: bool,
    ) -> bool {
        let disabled = attribute_value(attributes, "disabled").is_some();
        let checked = attribute_value(attributes, "checked").is_some();
        self.matches_identity(tag, attributes)
            && self.matches_state(hovered, disabled, checked)
            && self
                .nth_child
                .is_none_or(|expression| expression.matches(sibling_index))
    }

    fn matches_identity(&self, tag: &str, attributes: &HtmlAttributes) -> bool {
        self.tag
            .as_ref()
            .is_none_or(|name| name.eq_ignore_ascii_case(tag))
            && self
                .classes
                .iter()
                .all(|class| has_class(attributes, class))
            && self
                .id
                .as_ref()
                .is_none_or(|id| attribute_value(attributes, "id").is_some_and(|value| value == id))
            && self
                .attributes
                .iter()
                .all(|selector| selector.matches(attributes))
            && (!self.root || tag.eq_ignore_ascii_case("html"))
    }

    fn matches_state(&self, hovered: bool, disabled: bool, checked: bool) -> bool {
        (!self.hovered || hovered)
            && (!self.disabled || disabled)
            && (!self.not_disabled || !disabled)
            && (!self.checked || checked)
    }
}

impl CssNthExpression {
    pub(super) fn matches(self, sibling_index: usize) -> bool {
        let Ok(index) = i32::try_from(sibling_index) else {
            return false;
        };
        if index < 1 {
            return false;
        }
        if self.step == 0 {
            return index == self.offset;
        }
        let delta = index - self.offset;
        delta % self.step == 0 && delta / self.step >= 0
    }
}

impl CssAttributeSelector {
    fn matches(&self, attributes: &HtmlAttributes) -> bool {
        attribute_value(attributes, &self.name).is_some_and(|candidate| {
            self.value
                .as_ref()
                .is_none_or(|expected| candidate == expected)
        })
    }
}

fn has_class(attributes: &HtmlAttributes, class: &str) -> bool {
    attribute_value(attributes, "class").is_some_and(|classes| {
        classes
            .split_whitespace()
            .any(|candidate| candidate == class)
    })
}

fn attribute_value<'a>(attributes: &'a HtmlAttributes, name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
