use std::any::Any;

/// Type-erased contiguous column for one component type inside an Archetype.
pub(crate) trait ErasedVec: 'static {
    fn push_any(&mut self, val: Box<dyn Any>);
    /// Swap-remove at `index`, returning the displaced value.
    fn swap_remove_any(&mut self, index: usize) -> Box<dyn Any>;
    /// Replace the value at `index` in-place.
    fn set_any(&mut self, index: usize, val: Box<dyn Any>);
    fn get_any(&self, index: usize) -> &dyn Any;
    fn get_any_mut(&mut self, index: usize) -> &mut dyn Any;
    fn len(&self) -> usize;
    #[allow(dead_code)]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Create a new empty column of the same concrete type.
    fn new_empty(&self) -> Box<dyn ErasedVec>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub(crate) struct TypedVec<T: 'static> {
    pub data: Vec<T>,
}

impl<T: 'static> TypedVec<T> {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
}

impl<T: 'static> ErasedVec for TypedVec<T> {
    fn push_any(&mut self, val: Box<dyn Any>) {
        self.data.push(
            *val.downcast::<T>()
                .expect("ErasedVec type mismatch on push"),
        );
    }
    fn swap_remove_any(&mut self, index: usize) -> Box<dyn Any> {
        Box::new(self.data.swap_remove(index))
    }
    fn set_any(&mut self, index: usize, val: Box<dyn Any>) {
        self.data[index] = *val.downcast::<T>().expect("ErasedVec type mismatch on set");
    }
    fn get_any(&self, index: usize) -> &dyn Any {
        &self.data[index]
    }
    fn get_any_mut(&mut self, index: usize) -> &mut dyn Any {
        &mut self.data[index]
    }
    fn len(&self) -> usize {
        self.data.len()
    }
    fn new_empty(&self) -> Box<dyn ErasedVec> {
        Box::new(TypedVec::<T>::new())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
