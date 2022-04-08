#[derive(Debug, Clone)]
pub struct Scratchpad {
    command: String,
    client:  Cell<Option<Window>>,
    active:  Cell<bool>,
}
