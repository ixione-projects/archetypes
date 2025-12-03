#[macro_export]
macro_rules! set_configuration_options {
    ($l:expr,$($opt:expr),+) => {
        $(
            $l.configure($opt)?;
        )*
    };
}

#[macro_export]
macro_rules! result {
    ($e:expr) => {{
        let ret = $e;
        if ret < 0 {
            Err(Errno::from_inner(ret))
        } else {
            Ok(())
        }
    }};
}
