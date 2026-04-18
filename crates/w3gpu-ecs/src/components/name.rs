#[derive(Clone, Debug)]
pub struct NameComponent {
    pub name: String,
}

impl NameComponent {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
