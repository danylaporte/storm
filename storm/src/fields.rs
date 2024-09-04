use std::{
    any::{Any, TypeId},
    fmt::{Debug, Display},
};

pub trait Fields: Any + Debug + Display + Send + Sync {}

impl dyn Fields + 'static {
    pub fn is<T: Fields + 'static>(&self) -> bool {
        // Get `TypeId` of the type this function is instantiated with.
        let t = TypeId::of::<T>();

        // Get `TypeId` of the type in the trait object (`self`).
        let concrete = self.type_id();

        // Compare both `TypeId`s on equality.
        t == concrete
    }

    pub fn downcast_ref<T: Fields + 'static>(&self) -> Option<&T> {
        if self.is::<T>() {
            // SAFETY: `is` ensures this type cast is correct
            unsafe { Some(&*(self as *const dyn Fields as *const T)) }
        } else {
            None
        }
    }

    pub fn downcast_mut<T: Fields + 'static>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            // SAFETY: `is` ensures this type cast is correct
            unsafe { Some(&mut *(self as *mut dyn Fields as *mut T)) }
        } else {
            None
        }
    }
}
