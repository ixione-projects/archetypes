#[macro_export]
macro_rules! set_loop_options {
    ($l:expr,$($opt:expr),+) => {
        $(
        $l.configure($opt)?;
        )*
    };
}
