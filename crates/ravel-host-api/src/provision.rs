use ravel_core::Version;

/// A supplied library and its already parsed version.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Provided {
    library: String,
    version: Version,
}

impl Provided {
    /// Declares `library` supplied at `version`.
    ///
    /// Raw version text must be parsed before declaring a provider.
    ///
    /// ```compile_fail
    /// use ravel_host_api::Provided;
    ///
    /// let invalid = Provided::new("shared_maths", "not-a-version");
    /// ```
    pub fn new(library: impl Into<String>, version: Version) -> Self {
        Self {
            library: library.into(),
            version,
        }
    }

    /// Returns the library's name.
    #[must_use]
    pub fn library(&self) -> &str {
        &self.library
    }

    /// Returns the parsed version the provider declared.
    #[must_use]
    pub const fn version(&self) -> &Version {
        &self.version
    }

    /// Consumes the declaration, returning its owned library name and version.
    #[must_use]
    pub fn into_parts(self) -> (String, Version) {
        (self.library, self.version)
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::Version;

    use super::Provided;

    #[test]
    fn a_declaration_keeps_the_library_and_version_it_was_written_with() {
        let version = Version::new(2, 1, 0);
        let provided = Provided::new("shared_maths", version.clone());

        assert_eq!(provided.library(), "shared_maths");
        assert_eq!(provided.version(), &version);

        let (library, declared_version) = provided.into_parts();

        assert_eq!(library, "shared_maths");
        assert_eq!(declared_version, version);
    }
}
