#![allow(dead_code)]

use std::ops::{Index, IndexMut};

pub struct MultiSlice<T> {
    backing: Vec<Vec<T>>,
}

impl<T> MultiSlice<T> {
    pub fn new(backing: Vec<Vec<T>>) -> MultiSlice<T> {
        MultiSlice { backing }
    }

    pub fn len(&self) -> usize {
        self.backing.len()
    }

    pub fn get(&self, index: usize) -> Option<&[T]> {
        self.backing.get(index).map(|vec| &vec[..])
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut [T]> {
        self.backing.get_mut(index).map(|vec| &mut vec[..])
    }
}

impl<T> Index<usize> for MultiSlice<T> {
    type Output = [T];

    fn index(&self, index: usize) -> &Self::Output {
        &self.backing[index]
    }
}

impl<T> IndexMut<usize> for MultiSlice<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.backing[index]
    }
}
