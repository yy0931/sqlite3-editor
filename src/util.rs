/// `x.try_into().unwrap()`
pub fn into<T, U>(x: T) -> U
where
    T: TryInto<U>,
    <T as std::convert::TryInto<U>>::Error: std::fmt::Debug,
{
    TryInto::<U>::try_into(x).unwrap()
}
