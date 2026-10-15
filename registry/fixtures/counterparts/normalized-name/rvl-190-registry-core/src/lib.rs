//! A registry a counterpart owns. Every name here is this package's own.

pub struct Cyclops_Core {
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
