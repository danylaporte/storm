#[cfg(feature = "postgres")]
macro_rules! continue_ts {
    ($v:expr, $errors:ident) => {
        match $v {
            Ok(v) => v,
            Err(e) => {
                $errors.push(e);
                continue;
            }
        }
    };
}

macro_rules! try_ts {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            Err(e) => return e,
        }
    };
}
