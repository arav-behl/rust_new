pub mod order;
pub mod execution;
pub mod market;
pub mod metrics;

pub use order::*;
pub use execution::*;
pub use market::*;
pub use metrics::*;

// Type aliases for compatibility
pub type Price = rust_decimal::Decimal;
pub type Quantity = rust_decimal::Decimal;

// Import the ToPrimitive trait for to_f64 support
pub use rust_decimal::prelude::ToPrimitive;

// Orders module compatibility
pub mod orders {
    pub use super::order::*;
}