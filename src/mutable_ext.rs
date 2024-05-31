use futures_signals::signal;

pub trait MutableExt<T> {
    fn replace_pure<F>(&self, f: F)
    where
        T: Copy,
        F: FnOnce(T) -> T;

    // fn replace_pure_clone<F>(&self, f: F)
    // where
    //     T: Clone,
    //     F: FnOnce(T) -> T;
}

impl<T> MutableExt<T> for signal::Mutable<T> {
    /// Mutate the value of the signal by applying a function to it.
    fn replace_pure<F>(&self, f: F)
    where
        T: Copy,
        F: FnOnce(T) -> T,
    {
        let mut value = self.lock_mut();
        *value = f(*value);
    }

    // fn replace_pure_clone<F>(&self, f: F)
    // where
    //     T: Clone,
    //     F: FnOnce(T) -> T,
    // {
    //     let mut value = self.lock_mut();
    //     *value = f(value.clone());
    // }
}

