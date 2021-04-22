use version_tag::VersionTag;

pub trait Tag {
    fn tag(&self) -> VersionTag;
}

impl<T: Tag> Tag for &T {
    fn tag(&self) -> VersionTag {
        (**self).tag()
    }
}
