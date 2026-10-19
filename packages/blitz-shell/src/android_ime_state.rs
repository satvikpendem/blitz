//! Android uses UTF-16 offsets; the DOM editor uses UTF-8 byte boundaries.
use blitz_traits::events::BlitzImeEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AndroidEditorState {
    pub text: String,
    pub selection: (i32, i32),
    pub composition: Option<(i32, i32)>,
}

pub(crate) fn byte_to_utf16(text: &str, byte: usize) -> Option<i32> {
    let prefix = text.get(..byte)?;
    prefix.encode_utf16().count().try_into().ok()
}
fn utf16_to_byte(text: &str, offset: i32) -> Option<usize> {
    let offset = usize::try_from(offset).ok()?;
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Some(byte);
        }
        units += ch.len_utf16();
        if units > offset {
            return None;
        }
    }
    (units == offset).then_some(text.len())
}
impl AndroidEditorState {
    pub fn into_ime_event(self) -> Option<BlitzImeEvent> {
        let selection = (
            utf16_to_byte(&self.text, self.selection.0)?,
            utf16_to_byte(&self.text, self.selection.1)?,
        );
        let composition = match self.composition {
            Some((start, end)) if start >= 0 && end > start => Some((
                utf16_to_byte(&self.text, start)?,
                utf16_to_byte(&self.text, end)?,
            )),
            Some(_) => return None,
            None => None,
        };
        Some(BlitzImeEvent::SetState {
            text: self.text,
            selection,
            composition,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_round_trips_scalar_boundaries_and_preserves_reverse_selection() {
        let text = "Aé東京🦀Z";
        for byte in (0..=text.len()).filter(|&byte| text.is_char_boundary(byte)) {
            let units = byte_to_utf16(text, byte).unwrap();
            assert_eq!(utf16_to_byte(text, units), Some(byte));
        }
        let state = AndroidEditorState {
            text: text.into(),
            selection: (6, 4),
            composition: Some((2, 4)),
        };
        assert_eq!(
            state.into_ime_event(),
            Some(BlitzImeEvent::SetState {
                text: text.into(),
                selection: (13, 9),
                composition: Some((3, 9))
            })
        );
    }
    #[test]
    fn invalid_utf16_surrogate_boundaries_and_ranges_are_rejected() {
        for (selection, composition) in [
            ((5, 5), None),
            ((-1, 0), None),
            ((99, 99), None),
            ((0, 0), Some((6, 4))),
            ((0, 0), Some((4, 5))),
            ((0, 0), Some((-1, 6))),
        ] {
            assert!(
                AndroidEditorState {
                    text: "Aé東京🦀Z".into(),
                    selection,
                    composition
                }
                .into_ime_event()
                .is_none()
            );
        }
    }
}
