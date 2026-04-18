#[derive(Clone, Debug)]
pub struct NameComponent {
    pub name: String,
}

impl NameComponent {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_name() {
        let n = NameComponent::new("hero");
        assert_eq!(n.name, "hero");
    }

    #[test]
    fn new_accepts_string() {
        let n = NameComponent::new(String::from("enemy"));
        assert_eq!(n.name, "enemy");
    }
}
