mod errors;
pub mod pathmap;

type GhostSeedResult<T> = Result<T, errors::Error>;