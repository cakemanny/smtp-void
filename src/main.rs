
use std::net::{TcpListener, TcpStream};
use std::io::{BufReader, BufRead, Write};
use std::thread;

const MAX_MAIL_SIZE: u32 = 14680064;
const DOMAIN_NAME: &'static str = "mail.example.com";
const RESP_221: &'static [u8] =
    b"221 Bye\r\n";
const RESP_252: &'static [u8] =
    b"252 Cannot VRFY user, but will accept message and attempt delivery\r\n";
const RESP_354: &'static [u8] =
    b"354 End data with <CR><LF>.<CR><LF>\r\n";

fn main() {
    let listener = TcpListener::bind("127.0.0.1:2525").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    if let Err(e) = handle_connection(stream) {
                        println!("{}", e)
                    }
                });
            },
            Err(e) => {
                println!("Some error: {}", e)
            }
        }
    }
}

// Track both our transaction state and our data in our mail type
enum Mail {
    Empty,
    WithFrom(String),
    WithTo { from: String, tos: Vec<String> },
    WithData { from: String, tos: Vec<String>, data: String },
}

/// Removes the trailing new line from bytestring
///
/// # Examples
/// ```
/// let line1 = b"with cr lf\r\n";
/// assert_eq!(b"with cr lf", strip_newline(line1));
///
/// let line2 = b"with lf only\n";
/// assert_eq!(b"with lf only", strip_newline(line2));
/// ```
fn strip_newline(bytes: &Vec<u8>) -> &[u8] {
    if bytes.len() >= 2 && bytes[bytes.len() - 2] == b'\r' {
        &bytes[.. (bytes.len() - 2)]
    } else {
        &bytes[.. (bytes.len() - 1)]
    }
}


/*
 * Our response messages:
 */

fn helo_response(param: &[u8]) -> Vec<u8> {
    if !param.is_empty() {
        let param_txt = String::from_utf8_lossy(param);
        let res = format!("250 Hello {}, glad to meet you\r\n", param_txt);
        res.into_bytes()
    } else {
        b"250 Hello there, glad to meet you\r\n".to_vec()
    }
}

fn ehlo_response(param: &[u8]) -> Vec<u8> {
    let him = if param.is_empty() {
        String::from("you")
    } else {
        String::from(String::from_utf8_lossy(param))
    };

    let res = format!("250-{} Hello {}\r\n250 SIZE {}\r\n",
                        DOMAIN_NAME, him, MAX_MAIL_SIZE);
    res.into_bytes()
}
// TODO: these could all be written as constants
fn error_500(mut out: &TcpStream) -> std::io::Result<()> {
    try!(out.write(b"500 Syntax error, command unrecognised\r\n"));
    Ok(())
}
fn error_503(mut out: &TcpStream) -> std::io::Result<()> {
    try!(out.write(b"500 Bad sequence of commands\r\n"));
    Ok(())
}
fn ready_220(mut out: &TcpStream) -> std::io::Result<()> {
    try!(out.write(&format!("220 {} Service ready\r\n", DOMAIN_NAME).into_bytes()));
    Ok(())
}
fn ok_250(mut out: &TcpStream) -> std::io::Result<()> {
    try!(out.write(b"250 Ok\r\n"));
    Ok(())
}

// The handler for a single client connection from start to finish
fn handle_connection(stream: TcpStream) -> std::io::Result<()> {
    println!("Handling stream");
    // Start with a greeting
    try!(ready_220(&stream));

    let mut current_mail = Mail::Empty;

    // First attempt: read 1 command then quit
    // read until we reach end of line
    loop {
        let mut reader = BufReader::new(&stream);
        let mut line_bytes = Vec::new();
        let bytes_read = reader.read_until(b'\n', &mut line_bytes)?;
        if bytes_read <= 0 {
            break;
        }
        // Check for <CR><LF> vs <LF>
        let line_stripped: &[u8] = strip_newline(&line_bytes);
        // COMMAND SPACE PARAM
        let (command_bytes, param) = match line_stripped.iter().position(|&b| b == b' ') {
            Some(space_pos) => line_stripped.split_at(space_pos),
            None            => line_stripped.split_at(line_stripped.len())
        };

        let command = String::from_utf8_lossy(command_bytes);
        println!("Command: {}", command);
        if !param.is_empty() {
            println!("Param: {}", String::from_utf8_lossy(param));
        } else {
            println!("No param!");
        }

        {
            let mut out = &stream;
            match command.trim() {
                "EHLO" => {
                    let res = ehlo_response(param);
                    try!(out.write(&res));
                }
                "HELO" => {
                    let res = helo_response(param);
                    try!(out.write(&res));
                },
                "MAIL" => {
                    let param_txt = String::from_utf8_lossy(param);
                    if param_txt.len() < " FROM:?".len()
                            || !param_txt.starts_with(" FROM:") {
                        try!(error_500(out));
                    } else {
                        let from = &param_txt[6..];
                        println!("From: {}", from);

                        // Now should we try to process commands
                        current_mail = Mail::WithFrom(from.to_owned());
                        try!(ok_250(out));
                    }
                }
                "RCPT" => {
                    let param_txt = String::from_utf8_lossy(param);
                    if param_txt.len() < " TO:?".len()
                            || !param_txt.starts_with(" TO:") {
                        try!(error_500(out));
                    } else {
                        let to = &param_txt[4..];
                        println!("To: {}", to);

                        match current_mail {
                            Mail::WithFrom(from) => {
                                current_mail = Mail::WithTo {
                                    from: from,
                                    tos: vec![to.to_owned()]
                                };
                                try!(ok_250(out));
                            },
                            Mail::WithTo { from, mut tos } => {
                                tos.push(to.to_owned());
                                current_mail = Mail::WithTo {
                                    from: from,
                                    tos: tos
                                };
                                try!(ok_250(out));
                            },
                            _   => try!(error_503(out)),
                        }
                    }
                }
                "DATA" => {
                    match current_mail {
                        Mail::WithTo { from, tos } => {
                            try!(out.write(RESP_354));
                            let data = read_data(&mut reader)?;
                            println!("data: {}", data);
                            current_mail = Mail::WithData {
                                from: from,
                                tos: tos,
                                data: data
                            };
                            // TODO: write data somewhere durable
                            // such as a mysql database
                            try!(ok_250(out));
                        }
                        _   => try!(error_503(out)),
                    }
                }
                "RSET" => {
                    current_mail = Mail::Empty;
                    try!(ok_250(out));
                }
                "NOOP" => try!(ok_250(out)),
                "QUIT" => {
                    try!(out.write(RESP_221));
                    break;
                }
                "VRFY" => {
                    try!(out.write(RESP_252));
                }
                _      => try!(error_500(out)),
            }
        }
    }
    Ok(())
}

/// Read data until client signals end of data <CR><LF>.<CR><LF>
fn read_data(reader: &mut BufReader<&TcpStream>) -> std::io::Result<String> {
    let mut result = String::new();
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read <= 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Data ended before end of message"
            ));
        }
        if line == ".\r\n" || line == ".\n" {
            break;
        } else {
            result += &line;
        }
    }
    Ok(result)
}

