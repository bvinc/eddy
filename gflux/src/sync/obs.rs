use std::fmt;

type Callback<T> = Box<dyn Fn(&T) + Send + Sync>;

pub struct Obs<T> {
    value: T,
    callbacks: Vec<Callback<T>>,
}

impl<T> Obs<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            callbacks: vec![],
        }
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut T {
        self.trigger();
        &mut self.value
    }

    pub fn observe<F: 'static + Fn(&T) + Send + Sync>(&mut self, callback: F) {
        self.callbacks.push(Box::new(callback));
    }

    fn trigger(&mut self) {
        for cb in self.callbacks.iter_mut() {
            cb(&mut self.value);
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Obs<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Obs").field("value", &self.value).finish()
    }
}
