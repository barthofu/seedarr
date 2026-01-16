mod errors;
pub mod pathmap;

pub use errors::Error;

type SeedarrResult<T> = Result<T, Error>;
