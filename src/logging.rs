use std::fmt;

use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{self as tracing_fmt, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
};

fn restore_ansi_sequences(input: &str, scratch: &mut String) -> bool {
    if !input.contains("\\x") {
        return false;
    }

    scratch.clear();
    scratch.reserve(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 4 <= bytes.len() && bytes[i] == b'\\' && bytes[i + 1] == b'x' {
            let code = &bytes[i + 2..i + 4];
            match code {
                b"1b" | b"1B" => {
                    scratch.push('\x1b');
                    i += 4;
                    continue;
                }
                b"07" => {
                    scratch.push('\x07');
                    i += 4;
                    continue;
                }
                b"08" => {
                    scratch.push('\x08');
                    i += 4;
                    continue;
                }
                b"0c" | b"0C" => {
                    scratch.push('\x0c');
                    i += 4;
                    continue;
                }
                b"7f" | b"7F" => {
                    scratch.push('\x7f');
                    i += 4;
                    continue;
                }
                _ => {}
            }
        }
        scratch.push(bytes[i] as char);
        i += 1;
    }
    true
}

pub struct AnsiPreservingFormatter<F> {
    inner: F,
}

struct AnsiEscapeRestorer<'a> {
    writer: tracing_fmt::format::Writer<'a>,
    scratch: String,
}

impl<'a> fmt::Write for AnsiEscapeRestorer<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if restore_ansi_sequences(s, &mut self.scratch) {
            self.writer.write_str(&self.scratch)
        } else {
            self.writer.write_str(s)
        }
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.writer.write_char(c)
    }
}

impl<F> AnsiPreservingFormatter<F> {
    pub fn new(inner: F) -> Self {
        Self { inner }
    }
}

impl<S, N, F> FormatEvent<S, N> for AnsiPreservingFormatter<F>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
    N: for<'a> FormatFields<'a> + 'static,
    F: FormatEvent<S, N>,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: tracing_fmt::format::Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> fmt::Result {
        let mut adapter = AnsiEscapeRestorer {
            writer,
            scratch: String::new(),
        };
        let proxy = tracing_fmt::format::Writer::new(&mut adapter);
        self.inner.format_event(ctx, proxy, event)
    }
}
