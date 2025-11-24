pub trait Model {
    fn view(&self) -> Box<[u8]>;
}
