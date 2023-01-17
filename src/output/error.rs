#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OutputErrorKind {
    //PrepareError(String),
    WriteError(String),
    //CleanupError(String),
}

#[derive(Debug)]
pub struct OutputError {
    pub kind: OutputErrorKind,
    pub cause: Option<Box<dyn (::std::error::Error)>>,
}

impl ::std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match self.kind {
            //OutputErrorKind::PrepareError(ref s) => write!(f, "failed to prepare output {}", s),
            OutputErrorKind::WriteError(ref s) => {
                write!(f, "failed writing values to output {}", s)
            } //OutputErrorKind::CleanupError(ref s) => {
              //    write!(f, "cleanup of output {} returned an error", s)
              //}
        }
    }
}
