#[cfg(test)]
mod tests {
    use crate::renderer::backends::html_interactive::style::CssOverflow;
    use crate::renderer::backends::html_interactive::style::{CssFloat, CssPosition, CssStyle};

    const DEFAULT_BORDER_WIDTH: f32 = 4.0;
    const DEFAULT_ROTATION_DEGREES: f32 = 90.0;
    const INVALID_ROTATION_EPSILON: f32 = 0.001;
    const BORDER_WIDTH_NONE: f32 = 0.0;
    const BORDER_WIDTH_WITH_COLOR: f32 = 1.0;
    const BORDER_WIDTH_WITH_UNIT: f32 = 2.0;
    const BORDER_WIDTH_PRESERVED: f32 = 7.0;

    #[test]
    fn border_none_clears_border_state() {
        let mut style = CssStyle::browser_default();
        style.border = Some("blue".to_string());
        style.border_width = DEFAULT_BORDER_WIDTH;

        style.apply_border("none");

        assert!(style.border.is_none());
        assert_eq!(style.border_width, BORDER_WIDTH_NONE);
    }

    #[test]
    fn border_without_explicit_width_defaults_to_one_when_color_is_set() {
        let mut style = CssStyle::browser_default();
        style.apply_border("#123");

        assert_eq!(style.border, Some("#123".to_string()));
        assert_eq!(style.border_width, BORDER_WIDTH_WITH_COLOR);
    }

    #[test]
    fn border_with_length_parses_width_component() {
        let mut style = CssStyle::browser_default();
        style.apply_border("2px solid #123");

        assert_eq!(style.border, Some("#123".to_string()));
        assert_eq!(style.border_width, BORDER_WIDTH_WITH_UNIT);
    }

    #[test]
    fn invalid_border_preserves_width_when_no_color_or_length_is_present() {
        let mut style = CssStyle::browser_default();
        style.border_width = BORDER_WIDTH_PRESERVED;

        style.apply_border("solid");

        assert!(style.border.is_none());
        assert_eq!(style.border_width, BORDER_WIDTH_PRESERVED);
    }

    #[test]
    fn apply_dispatches_to_box_or_typography_rules() {
        let mut style = CssStyle::browser_default();
        style.apply("overflow", "hidden");
        assert_eq!(style.overflow, CssOverflow::Clip);
        style.apply("font-family", "Inter");
        assert_eq!(style.font_family, "Inter");
        style.apply("unknown-property", "42");
    }

    #[test]
    fn outer_box_shadow_keeps_offsets_blur_spread_and_function_color() {
        let mut style = CssStyle::browser_default();
        style.apply("box-shadow", "2px 10px 28px 3px rgba(15, 40, 89, 0.14)");

        assert_eq!(
            style.box_shadow.as_ref().map(|shadow| (
                shadow.offset_x,
                shadow.offset_y,
                shadow.blur_radius,
                shadow.spread_radius,
                shadow.color.as_str(),
            )),
            Some((2.0, 10.0, 28.0, 3.0, "rgba(15, 40, 89, 0.14)"))
        );

        style.apply("box-shadow", "inset 0 0 2px red");
        assert!(style.box_shadow.is_some());
        style.apply("box-shadow", "none");
        assert!(style.box_shadow.is_none());
    }

    #[test]
    fn list_style_none_is_inherited_and_can_be_restored() {
        let mut parent = CssStyle::browser_default();
        parent.apply("list-style", "none");
        assert!(parent.list_style_none);

        let child = CssStyle::from_attributes(&[], &parent);
        assert!(child.list_style_none);

        parent.apply("list-style-type", "disc");
        assert!(!parent.list_style_none);
        parent.apply("list-style", "invalid");
        assert!(!parent.list_style_none);
        parent.apply("list-style", "initial");
        assert!(!parent.list_style_none);
    }

    #[test]
    fn positioning_declarations_handle_auto_invalid_and_inline_grid() {
        let mut style = CssStyle::browser_default();
        style.apply("z-index", "4");
        style.apply("z-index", "auto");
        assert!(style.z_index.is_none());

        style.apply("z-index", "invalid");
        style.apply("position", "invalid");
        assert_eq!(style.position, CssPosition::Static);

        style.apply("display", "inline-grid");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(style.inline_block);
        assert!(style.inline_atomic);
    }

    #[test]
    fn float_declarations_accept_left_right_and_none_without_inheritance() {
        let mut style = CssStyle::browser_default();
        style.apply("float", "left");
        assert_eq!(style.float, CssFloat::Left);
        assert_eq!(CssStyle::from_attributes(&[], &style).float, CssFloat::None);

        style.apply("float", "right");
        assert_eq!(style.float, CssFloat::Right);
        style.apply("float", "invalid");
        assert_eq!(style.float, CssFloat::Right);
        style.apply("float", "none");
        assert_eq!(style.float, CssFloat::None);
    }

    #[test]
    fn appearance_and_rotation_are_parsed_without_inheritance() {
        let mut style = CssStyle::browser_default();
        style.apply("appearance", "none");
        style.apply("-webkit-transform", "rotate(0.25turn)");

        assert!(style.appearance_none);
        assert_eq!(style.rotation_degrees, DEFAULT_ROTATION_DEGREES);
        let child = CssStyle::from_attributes(&[], &style);
        assert!(!child.appearance_none);
        assert_eq!(child.rotation_degrees, 0.0);

        style.apply("transform", "rotate(3.1415927rad)");
        assert!((style.rotation_degrees - 180.0).abs() < INVALID_ROTATION_EPSILON);
        style.apply("transform", "invalid");
        assert!((style.rotation_degrees - 180.0).abs() < INVALID_ROTATION_EPSILON);
        style.apply("transform", "none");
        style.apply("appearance", "auto");
        assert_eq!(style.rotation_degrees, 0.0);
        assert!(!style.appearance_none);
    }

    #[test]
    fn inline_block_preserves_shrink_to_fit_display_semantics() {
        let mut style = CssStyle::browser_default();

        style.apply("display", "inline-block");
        assert_eq!(style.display, taffy::style::Display::Block);
        assert!(style.inline_block);
        assert!(style.inline_atomic);

        style.apply("display", "inline");
        assert_eq!(style.display, taffy::style::Display::Block);
        assert!(style.inline_block);
        assert!(!style.inline_atomic);

        style.apply("display", "inline-flex");
        assert_eq!(style.display, taffy::style::Display::Flex);
        assert!(style.inline_block);
        assert!(style.inline_atomic);
    }

    #[test]
    fn block_display_clears_inline_shrink_to_fit_display_semantics() {
        let mut style = CssStyle::browser_default();
        style.apply("inline", "inline-flex");

        style.apply("display", "grid");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(!style.inline_block);

        style.apply("display", "unsupported");
        assert_eq!(style.display, taffy::style::Display::Grid);
        assert!(!style.inline_block);
    }
}
