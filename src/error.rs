pub type OrchResult<T, E = OrchError> = Result<T, E>;

#[derive(Debug)]
pub enum OrchError {
    Ec2 { dbg: String },
    Iam { dbg: String },
    Ssm { dbg: String },
}

impl std::fmt::Display for OrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrchError::Ec2 { dbg } => write!(f, "{}", dbg),
            OrchError::Iam { dbg } => write!(f, "{}", dbg),
            OrchError::Ssm { dbg } => write!(f, "{}", dbg),
        }
    }
}

impl std::error::Error for OrchError {}
