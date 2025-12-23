use rustc_span::{Symbol, sym};
use rustc_macros::HashStable;

#[derive(Copy, Clone, Debug, Eq, Hash, HashStable)]
pub struct CompartmentsBuffer(#[allow(unused)] [Symbol; 10]);

impl CompartmentsBuffer {
    #[allow(unused)]
    pub fn new() -> Self {
        Self([sym::dummy; 10])
    }

    pub fn from(arr: [Symbol; 10]) -> Self {
        Self(arr)
    }

    pub fn from_slice(slice: &[Symbol]) -> Self {
        let mut arr = [sym::dummy; 10];
        let len = slice.len().min(10);
        arr[..len].copy_from_slice(&slice[..len]);
        Self(arr)
    }

    pub fn get(&self) -> [Symbol; 10] {
        self.0
    }

    pub fn set(&mut self, arr: [Symbol; 10]) {
        self.0 = arr;
    }

    pub fn first_dummy_index(&self) -> usize {
        self.0.iter().position(|s| *s == sym::dummy).unwrap()
    }

    pub fn push_to_index(&mut self, index: usize, symbol: Symbol) {
        self.0[index] = symbol;
    }
}

impl PartialEq for CompartmentsBuffer {
    fn eq(&self, other: &Self) -> bool {
        fn has_non_dummy(inp: [Symbol; 10]) -> bool {
            inp.iter().any(|s| *s != sym::dummy)
        }

        fn has_common_non_dummy<const N: usize>(a: [Symbol; N], b: [Symbol; N]) -> bool {
            let mut i = 0;
            while i < N {
                if a[i] != sym::dummy {
                    let mut j = 0;
                    while j < N {
                        if b[j] != sym::dummy && a[i] == b[j] {
                            return true;
                        }
                        j += 1;
                    }
                }
                i += 1;
            }
            false
        }

        match (has_non_dummy(self.0), has_non_dummy(other.0)) {
            (true, true) => has_common_non_dummy(self.0, other.0),
            (true, false) => true,
            (false, true) => true,
            (false, false) => true,
        }
    }
}

// pub trait Compartments {
//     fn get_compartments(&self) -> &CompartmentsBuffer;
//
//     fn set_compartments(&mut self, compartments: CompartmentsBuffer);
//
//     fn merge_slice(&mut self, slice: &[Symbol]);
// }
