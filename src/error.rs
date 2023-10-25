pub type OrchResult<T, E = OrchError> = Result<T, E>;

#[derive(Debug)]
pub enum OrchError {
    Ec2 { dbg: String },
}

impl std::fmt::Display for OrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for OrchError {}
