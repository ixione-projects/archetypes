pub(crate) trait FromInner<T>: Sized {
    fn from_inner(value: T) -> Self;
}

pub(crate) trait IntoInner<T>: Sized {
    fn into_inner(self) -> T;
}
