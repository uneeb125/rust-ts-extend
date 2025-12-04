use rustc_span::{Symbol, sym};

#[derive(Copy, Clone, Debug)]
pub struct CompartmentsBuffer {
    #[allow(unused)]
    data: [Symbol; 100],
}
impl CompartmentsBuffer {
    #[allow(unused)]
    pub fn new() -> Self {
        Self {
            data: [sym::dummy; 100],
        }
    }

    #[allow(unused)]
    pub fn push(&mut self, value: Symbol) {
        // find the index of first dummy value
        let index = self.data.iter().position(|a| *a == sym::dummy).unwrap_or_else(|| panic!("Compartments buffer is full"));
        self.data[index] = value;
    }

    #[allow(unused)]
    pub fn as_slice(&self) -> &[Symbol] {
        &self.data
    }
}

pub trait Compartments {
    fn get_compartments(&self) -> CompartmentsBuffer;

    fn set_compartments(&self, compartments: CompartmentsBuffer);
}

pub fn compartments_contains_same_symbol(a: CompartmentsBuffer, b: CompartmentsBuffer) -> bool {
    a.as_slice().iter().filter(|a| **a != sym::dummy).any(|a| b.as_slice().iter().filter(|b| **b != sym::dummy).any(|b| a == b))
}

pub fn has_non_dummy_compartments(compartments: CompartmentsBuffer) -> bool {
    compartments.as_slice().iter().any(|a| *a != sym::dummy)
}
