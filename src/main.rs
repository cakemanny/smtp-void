
#[macro_use]
extern crate mysql;
extern crate clap;

mod storage;

use std::net::{TcpListener, TcpStream};
use std::io::{BufReader, BufRead, Write};
use std::thread;
use clap::{Arg, App};

use storage::{Mail, Storage, DbStorage};

const MAX_MAIL_SIZE: u32 = 14680064;
const DOMAIN_NAME: &'static str = "mail.example.com";
const RESP_221: &'static [u8] =
    b"221 Bye\r\n";
const RESP_252: &'static [u8] =
    b"252 Cannot VRFY user, but will accept message and attempt delivery\r\n";
const RESP_354: &'static [u8] =
    b"354 End data with <CR><LF>.<CR><LF>\r\n";


fn main() {
    let matches = App::new("SMTP Void")
                        .version("0.1")
                        .author("Dan Golding")
                        .about("A dummy smtp server with ability to store messages to database")
                        .arg(Arg::with_name("bind-address")
                                .long("bind")
                                .value_name("BIND_ADDRESS")
                                .help("Address to listen for smtp connections, default: 0.0.0.0:25")
                                .takes_value(true))
                        .arg(Arg::with_name("mysql-url")
                                .long("mysql")
                                .value_name("MYSQL_URL")
                                .help("URL to mysql database in format mysql://{user}:{pass}@{host}:{port}/{database}")
                                .required(true)
                                .takes_value(true))
                        .get_matches();

    let bind_addr = matches.value_of("bind-address").unwrap_or("0.0.0.0:25");
    let listener = TcpListener::bind(bind_addr).unwrap();

    let mysql_url = matches.value_of("mysql-url").expect("mysql-url is required");
    let pool = mysql::Pool::new(mysql_url).unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let pool = pool.clone();
                thread::spawn(move || {
                    let store = DbStorage::new(pool);
                    if let Err(e) = handle_connection(stream, &store) {
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
fn handle_connection(stream: TcpStream, store: &Storage) -> std::io::Result<()> {
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
                },
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
                },
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
                },
                "DATA" => match current_mail {
                    Mail::WithTo { from, tos } => {
                        try!(out.write(RESP_354));
                        let data = read_data(&mut reader)?;
                        println!("data: {}", data);
                        current_mail = Mail::WithData {
                            from: from,
                            tos: tos,
                            data: data
                        };

                        // Store somewhere durable
                        match store.store_mail(&current_mail) {
                            Ok(_) => {
                                println!("Successfully stored mail");
                            }
                            _ => {
                                // TODO: should we send an error reply instead?
                                println!("Failed to store mail");
                            }
                        }

                        try!(ok_250(out));
                        current_mail = Mail::Empty;
                    }
                    _   => try!(error_503(out)),
                },
                "RSET" => {
                    current_mail = Mail::Empty;
                    try!(ok_250(out));
                },
                "NOOP" => try!(ok_250(out)),
                "QUIT" => {
                    try!(out.write(RESP_221));
                    break;
                },
                "VRFY" => {
                    try!(out.write(RESP_252));
                },
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




