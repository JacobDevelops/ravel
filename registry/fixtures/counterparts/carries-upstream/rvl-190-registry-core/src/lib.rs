//! A registry a counterpart owns. Ported from cyclopscore.

pub struct Registry {
    entries: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn push(&mut self, name: String) {
        self.entries.push(name);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
