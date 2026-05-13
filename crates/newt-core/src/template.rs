//! Prompt template rendering.
//!
//! Phase 1 supports a single placeholder, `{{selection}}`, which is replaced
//! verbatim by the input text. Phase 2 plans to add additional placeholders
//! (surrounding paragraphs, source app name) — at that point we may graduate
//! to a real templating crate, but for now plain string substitution is
//! enough and avoids pulling in a dependency.

/// Render a prompt template by substituting `{{selection}}` with the given
/// selection text.
pub fn render(template: &str, selection: &str) -> String {
    template.replace("{{selection}}", selection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_selection_placeholder() {
        let out = render("Rewrite this:\n\n{{selection}}\n", "hello world");
        assert_eq!(out, "Rewrite this:\n\nhello world\n");
    }

    #[test]
    fn template_without_placeholder_is_returned_unchanged() {
        let out = render("static prompt body", "ignored");
        assert_eq!(out, "static prompt body");
    }

    #[test]
    fn multiple_placeholders_all_replaced() {
        let out = render("{{selection}} / {{selection}}", "x");
        assert_eq!(out, "x / x");
    }
}
