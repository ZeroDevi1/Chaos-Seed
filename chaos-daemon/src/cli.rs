#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMode {
    NamedPipe { pipe_name: String },
    Stdio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub transport: TransportMode,
    pub auth_token: String,
}

impl CliOptions {
    pub fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut pipe_name: Option<String> = None;
        let mut auth_token: Option<String> = None;
        let mut stdio = false;

        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            let a = a.as_ref();
            match a {
                "--pipe-name" => pipe_name = it.next().map(|v| v.as_ref().to_string()),
                "--auth-token" => auth_token = it.next().map(|v| v.as_ref().to_string()),
                "--stdio" => stdio = true,
                _ => {}
            }
        }

        let auth_token = auth_token
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "missing --auth-token".to_string())?;

        if stdio {
            return Ok(Self {
                transport: TransportMode::Stdio,
                auth_token,
            });
        }

        let pipe_name = pipe_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "missing --pipe-name".to_string())?;

        Ok(Self {
            transport: TransportMode::NamedPipe { pipe_name },
            auth_token,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_named_pipe_mode_requires_pipe_and_token() {
        let err = CliOptions::parse(["--pipe-name", "p"]).unwrap_err();
        assert!(err.contains("auth-token"));

        let err = CliOptions::parse(["--auth-token", "t"]).unwrap_err();
        assert!(err.contains("pipe-name"));

        let ok = CliOptions::parse(["--pipe-name", "p", "--auth-token", "t"]).unwrap();
        assert_eq!(
            ok,
            CliOptions {
                transport: TransportMode::NamedPipe {
                    pipe_name: "p".to_string()
                },
                auth_token: "t".to_string()
            }
        );
    }

    #[test]
    fn parse_stdio_mode_requires_only_token() {
        let err = CliOptions::parse(["--stdio"]).unwrap_err();
        assert!(err.contains("auth-token"));

        let ok = CliOptions::parse(["--stdio", "--auth-token", "t"]).unwrap();
        assert_eq!(
            ok,
            CliOptions {
                transport: TransportMode::Stdio,
                auth_token: "t".to_string()
            }
        );
    }
}

