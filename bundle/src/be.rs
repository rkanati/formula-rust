use bytemuck::{self as bm, Pod, Zeroable};

#[repr(transparent)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Be<T>(T) where T: Pod;

impl<T> std::fmt::Debug for Be<T> where T: Pod + std::fmt::Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} (be)", self.get())
    }
}

impl<T> Be<T> where T: Pod {
    pub fn get(mut self) -> T {
        bm::bytes_of_mut(&mut self.0).reverse();
        self.0
    }
}

impl<T> From<T> for Be<T> where T: Pod {
    fn from(mut x: T) -> Self {
        bm::bytes_of_mut(&mut x).reverse();
        Be(x)
    }
}

