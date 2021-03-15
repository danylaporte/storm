#[macro_export]
macro_rules! version_deps {
    (
        let mut $version:ident = 0;
        $(let $v:ident = $e:expr;)+
    ) => {
        let mut $version = 0;
        $(let $v = storm::GetVersion::max($e, &mut $version);)*
    };
}
