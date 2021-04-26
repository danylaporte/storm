use crate::Entity;

pub trait EntityOf {
    type Entity: Entity;
}
