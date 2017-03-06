use std;
use toml;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Message(err: &'static str) {
            description(err)
            from()
        }
        Io(err: std::io::Error) {
            cause(err)
            description(err.description())
            from()
        }
        Toml(err: toml::de::Error) {
            cause(err)
            description(err.description())
            from()
        }
        Other(err: Box<std::error::Error>) {
            cause(&**err)
            description(err.description())
        }
    }
}
