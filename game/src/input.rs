#[derive(Default, Clone, Copy)]
pub struct Bipole(i32, i32);

impl Bipole {
    pub fn pos(&mut self, go: bool) {
        if go {self.1 = self.0 + 1}
        else  {self.1 = 0};
    }

    pub fn neg(&mut self, go: bool) {
        if go {self.0 = self.1 + 1}
        else  {self.0 = 0};
    }

    pub fn eval(self) -> i32 {
        (self.1 - self.0).signum()
    }
}

pub enum Button { Forward, Back, Left, Right, Ascend, Descend, Fast, }

