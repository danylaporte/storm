#[macro_export]
macro_rules! version_deps {
    (
        let $version:ident = 0;
        $(let $v:ident = $e:expr;)+
    ) => {
        $(let $v = $e;)*
        let $version = storm::version_tag::combine(&[$(storm::Tag::tag(&$v),)*]);
    };
}
