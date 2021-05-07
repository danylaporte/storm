use syn::{PathSegment, Type};

pub trait TypeExt {
    fn is_type_of_segment(&self, idents: &[&str]) -> bool;

    fn is_cache_island(&self) -> bool {
        self.is_type_of_segment(&["cache", "CacheIsland"])
    }

    fn is_storm_ctx(&self) -> bool {
        self.is_type_of_segment(&["storm", "Ctx"])
    }
}

impl TypeExt for Type {
    fn is_type_of_segment(&self, idents: &[&str]) -> bool {
        match self {
            Type::Path(p) => check_path_segment(&p.path.segments, idents),
            _ => false,
        }
    }
}

fn check_path_segment<'a, SEGS>(segments: SEGS, idents: &[&str]) -> bool
where
    SEGS: IntoIterator<Item = &'a PathSegment>,
    SEGS::IntoIter: DoubleEndedIterator,
{
    segments
        .into_iter()
        .rev()
        .zip(idents.iter().rev())
        .all(|(a, b)| a.ident == b)
}
