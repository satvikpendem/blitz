use super::View;
use anyrender::WindowRenderer;
use blitz_traits::events::UiEvent;

impl<Rend: WindowRenderer> View<Rend> {
    pub(super) fn sync_android_ime(&mut self) {
        use crate::android_ime_state::{AndroidEditorState, byte_to_utf16};
        let snapshot = {
            let inner = self.doc.inner();
            inner.get_focussed_node_id().and_then(|id| {
                let element = inner.get_node(id)?.element_data()?;
                if element.has_attr(blitz_dom::local_name!("readonly"))
                    || element.has_attr(blitz_dom::local_name!("disabled"))
                {
                    return None;
                }
                let input = element.text_input_data()?;
                let editor = &input.editor;
                let text = editor.raw_text();
                let selection = (
                    byte_to_utf16(text, editor.raw_selection().anchor().index())?,
                    byte_to_utf16(text, editor.raw_selection().focus().index())?,
                );
                let composition = match editor.raw_compose() {
                    Some(range) => Some((
                        byte_to_utf16(text, range.start)?,
                        byte_to_utf16(text, range.end)?,
                    )),
                    None => None,
                };
                // Android EditorInfo flags; password editors disable learning and suggestions.
                let mut options = 0x0200_0000 | if input.is_multiline { 1 } else { 6 };
                let mut input_type = if input.is_multiline {
                    1 | 0x0002_0000
                } else {
                    1
                };
                match element
                    .attr(blitz_dom::local_name!("type"))
                    .unwrap_or("text")
                {
                    "password" => {
                        input_type |= 0x80 | 0x0008_0000;
                        options |= 0x0100_0000;
                    }
                    "email" => input_type |= 0x20,
                    "url" => input_type |= 0x10,
                    "number" => input_type = 2 | 0x1000 | 0x2000,
                    "tel" => input_type = 3,
                    _ => {}
                }
                Some((
                    id,
                    AndroidEditorState {
                        text: text.into(),
                        selection,
                        composition,
                    },
                    input_type,
                    options,
                ))
            })
        };
        let focus = snapshot.as_ref().map(|(id, ..)| *id);
        if self.android_ime_focus != focus {
            self.android_ime_focus = focus;
            self.android_ime_generation = self.android_ime_generation.wrapping_add(1).max(1);
            self.android_ime_acknowledged = 0;
        }
        let (session, state, input_type, options) = match snapshot {
            Some((_, state, input_type, options)) => {
                (self.android_ime_generation, state, input_type, options)
            }
            None => (
                0,
                AndroidEditorState {
                    text: String::new(),
                    selection: (0, 0),
                    composition: None,
                },
                0,
                0,
            ),
        };
        if let Err(error) = self.android_ime.update(
            session,
            self.android_ime_acknowledged,
            state,
            input_type,
            options,
        ) {
            eprintln!("Android editor synchronization failed: {error}");
        }
    }

    pub(crate) fn apply_android_ime(
        &mut self,
        monitor_id: u64,
        session: u64,
        sequence: u64,
        event: crate::AndroidImeEvent,
    ) {
        use crate::{AndroidImeEvent, android_ime_state::AndroidEditorState};
        use blitz_traits::events::{BlitzKeyEvent, KeyState};
        use keyboard_types::{Code, Key, Location, Modifiers};
        self.sync_android_ime();
        if monitor_id != self.android_ime.id()
            || self.android_ime_focus.is_none()
            || session != self.android_ime_generation
            || sequence < self.android_ime_acknowledged
        {
            return;
        }
        let event = match event {
            AndroidImeEvent::State {
                text,
                selection,
                composition,
            } => {
                if sequence == self.android_ime_acknowledged {
                    return;
                }
                self.android_ime_acknowledged = sequence;
                AndroidEditorState {
                    text,
                    selection,
                    composition,
                }
                .into_ime_event()
                .map(UiEvent::Ime)
            }
            AndroidImeEvent::Action(action) => {
                let (key, code, modifiers) = match action {
                    0..=4 | 6 => (Key::Enter, Code::Enter, Modifiers::empty()),
                    5 => (Key::Tab, Code::Tab, Modifiers::empty()),
                    7 => (Key::Tab, Code::Tab, Modifiers::SHIFT),
                    _ => return,
                };
                Some(UiEvent::KeyDown(BlitzKeyEvent {
                    key,
                    code,
                    modifiers,
                    location: Location::Standard,
                    is_auto_repeating: false,
                    is_composing: false,
                    state: KeyState::Pressed,
                    text: None,
                }))
            }
        };
        if let Some(event) = event {
            self.doc.handle_ui_event(event);
            self.poll();
        }
        self.sync_android_ime();
        self.request_redraw();
    }

    pub(super) fn show_android_ime(&mut self) {
        self.sync_android_ime();
        if self.android_ime_focus.is_some() {
            if let Err(error) = self.android_ime.show(self.android_ime_generation) {
                eprintln!("Android keyboard activation failed: {error}");
            }
        }
    }
}
