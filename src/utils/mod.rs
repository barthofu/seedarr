mod errors;
pub mod pathmap;

type SeedarrResult<T> = Result<T, errors::Error>;