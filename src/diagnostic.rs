use crate::content::ContentError;

pub(crate) fn terminal_safe(value: &str) -> String {
    let mut safe = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\n' | '\t') || !character.is_control() {
            safe.push(character);
        } else {
            safe.extend(character.escape_debug());
        }
    }
    safe
}

pub(crate) fn format_content_error(error: &ContentError) -> String {
    let item = error.item_id.as_deref().map_or(String::new(), |item| {
        format!(" item={}", item.escape_debug())
    });
    format!(
        "pack={}{} field={}: {}",
        error.pack_id.escape_debug(),
        item,
        error.field.escape_debug(),
        error.message.escape_debug()
    )
}
