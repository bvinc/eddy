use std::cell::RefCell;
use std::fmt;

type Callback<T> = Box<dyn Fn(&T)>;

pub struct Obs<T> {
    value: T,
    callbacks: RefCell<Vec<Callback<T>>>,
}

impl<T> Obs<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            callbacks: RefCell::new(vec![]),
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.trigger();
        &mut self.value
    }

    pub fn observe<F: 'static + Fn(&T)>(&self, callback: F) {
        self.callbacks.borrow_mut().push(Box::new(callback));
    }

    fn trigger(&mut self) {
        let callbacks = self.callbacks.get_mut();
        for cb in callbacks.iter_mut() {
            cb(&mut self.value);
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Obs<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Obs").field("value", &self.value).finish()
    }
}
